//! Utilities for creating/opening perf events.

use crate::api::{Counter, SampledCounter};
use crate::perf::ffi;
use crate::perf::PAGE_SIZE;
use crate::{Error, Result, ScaledValue};
use byteorder::NativeEndian;
use byteorder::ReadBytesExt;
use log::debug;
use nix::libc;
use std::convert::TryInto;
use std::os::unix::io::{AsRawFd, FromRawFd};

/// Individual values held by a `SampleEvent`.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PerfEventValue {
    /// Counter measurement.
    pub(crate) value: u64,
    /// Total time spent enabled.
    pub(crate) time_enabled: u64,
    /// Total time spent running.
    ///
    /// In the case the of event multiplexing the `time_enabled` and `time running` values can be
    /// used to scale an estimated value for the count.
    pub(crate) time_running: u64,
    /// Globally unique ID for the event.
    pub(crate) id: u64,
}

impl PerfEventValue {
    /// Parse a PerfEventValue from a memory buffer.
    pub fn from_bytes(ptr: &mut [u8; 32]) -> Result<Self> {
        Self::from_cursor(&mut std::io::Cursor::new(ptr))
    }

    /// Parse this structure from a serialized in-memory format provided by the kernel.
    ///
    /// The structure parsed here is based on the configuration of enclosing `PerfEvent`'s attribute
    /// set in `PerfEventBuilder::open`.
    pub fn from_cursor<T>(ptr: &mut std::io::Cursor<T>) -> Result<Self>
    where
        std::io::Cursor<T>: byteorder::ReadBytesExt,
    {
        Ok(PerfEventValue {
            value: ptr.read_u64::<NativeEndian>()?,
            time_enabled: ptr.read_u64::<NativeEndian>()?,
            time_running: ptr.read_u64::<NativeEndian>()?,
            id: ptr.read_u64::<NativeEndian>()?,
        })
    }
}

impl ScaledValue<u64> for PerfEventValue {
    fn raw_value(&self) -> u64 {
        self.value
    }

    fn scaled_value(&self) -> u64 {
        let v = self.value as f64;
        let r = self.time_running as f64;
        let e = self.time_enabled as f64;
        (v * r / e) as u64
    }
}

/// The equality check only compares raw values and not the scaled values.
impl PartialEq for PerfEventValue {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

/// A schedulable and readable performance counter.
///
/// Represents a readable perf event which can be used to collect data directly from the kernel,
/// through the memory mapped ring buffer, or through direct read from hardware.
#[derive(Debug)]
pub struct PerfEvent {
    /// Name of the event.
    name: String,
    /// Attributes corresponding to this event.
    pub attr: ffi::perf_event_attr,
    /// File corresponding to the underlying perf event.
    file: std::fs::File,
    /// Ring buffer corresponding to underlying perf event.
    pub(crate) ring_buffer: Option<crate::perf::RingBuffer>,
}

impl Counter<PerfEventValue> for PerfEvent {
    fn name(&self) -> &String {
        &self.name
    }

    fn enable(&self) -> Result<()> {
        unsafe {
            ffi::perf_event_ioc_enable(self.file.as_raw_fd())?;
        }
        debug!("PerfEvent enabled: {:?}", self.attr);
        Ok(())
    }

    fn disable(&self) -> Result<()> {
        unsafe {
            ffi::perf_event_ioc_disable(self.file.as_raw_fd())?;
        }
        debug!("PerfEvent disabled: {:?}", self.attr);
        Ok(())
    }

    fn reset(&self) -> Result<()> {
        unsafe {
            ffi::perf_event_ioc_reset(self.file.as_raw_fd())?;
        }
        debug!("PerfEvent reset: {:?}", self.attr);
        Ok(())
    }

    fn is_closed(&self) -> Result<bool> {
        let poll = self.poll(0)?;
        Ok(poll.intersects(nix::poll::PollFlags::POLLHUP))
    }

    fn read_sync(&self) -> Result<PerfEventValue> {
        let mut bytes = [0u8; 32];
        nix::unistd::read(self.file.as_raw_fd(), &mut bytes)?;
        PerfEventValue::from_bytes(&mut bytes)
    }
}

impl SampledCounter<PerfEventValue> for PerfEvent {
    fn read_samples(&mut self) -> Vec<PerfEventValue> {
        if let Some(ref mut rb) = self.ring_buffer {
            let mut num_evts = 0usize;
            let evts: Vec<PerfEventValue> = rb
                .events()
                .filter_map(|e| {
                    num_evts += 1;
                    if e.is_sample() {
                        match e.parse().unwrap() {
                            crate::perf::ParsedRecord::Sample(s) => Some(s.value),
                            _ => unreachable!(),
                        }
                    } else {
                        None
                    }
                })
                .collect();
            self.advance(Some(num_evts));
            evts
        } else {
            vec![]
        }
    }

    fn unread_events(&self, timeout: libc::c_int) -> Result<bool> {
        let p = self.poll(timeout)?;
        Ok(p.intersects(nix::poll::PollFlags::POLLIN))
    }

    fn advance(&mut self, num_items: Option<usize>) {
        if let Some(ref mut rb) = self.ring_buffer {
            rb.advance(num_items);
        }
    }
}

impl PerfEvent {
    /// Construct a new perf event using the associated builder,
    pub fn build() -> PerfEventBuilder {
        PerfEventBuilder::default()
    }

    /// Write modifications to `self.attr` to the kernel.
    pub fn modify_event_attributes(&mut self) -> Result<()> {
        unsafe {
            ffi::perf_event_ioc_modify_attributes(
                self.file.as_raw_fd(),
                &mut self.attr as *mut ffi::perf_event_attr,
            )?;
        }
        debug!("PerfEvent modified: {:?}", self.attr);
        Ok(())
    }

    /// Modify the frequency or period at which this event is sampled.
    ///
    /// By default the event is period based. To switch to frequency modify the event's `attr` field
    /// and write the modifications to the kernel using `modify_event_attributes`.
    pub fn modify_frequency_period(&mut self, period: u64) -> Result<()> {
        unsafe {
            ffi::perf_event_ioc_period(self.file.as_raw_fd(), period)?;
        }
        debug!("PerfEvent modified: {:?}", self.attr);
        Ok(())
    }

    /// Poll for new events.
    fn poll(&self, timeout: libc::c_int) -> Result<nix::poll::PollFlags> {
        let mut pollfd = [nix::poll::PollFd::new(
            self.file.as_raw_fd(),
            nix::poll::PollFlags::POLLIN | nix::poll::PollFlags::POLLHUP,
        )];
        nix::poll::poll(&mut pollfd, timeout)?;
        match pollfd[0].revents() {
            Some(x) => Ok(x),
            _ => unreachable!(),
        }
    }
}

/// Helper struct to build a `PerfEvent` object.
#[derive(Debug)]
pub struct PerfEventBuilder {
    /// Name of the event.
    name: String,
    /// Target process ID.
    ///
    /// Defaults to current process.
    pid: libc::pid_t,
    /// Target CPU ID.
    ///
    /// Defaults to all CPUs.
    cpuid: libc::c_int,
    /// File descriptor for the group leader event.
    ///
    /// Defaults to none.
    leader: libc::c_int,
    /// Use sampling frequency instead of sampling period.
    ///
    /// Defaults to `false`.
    use_freq: bool,
    /// Sampling frequency or period based on `use_freq`.
    ///
    /// Defaults to period `1`.
    freq_or_period: u64,
    /// Should
    ///
    /// Defaults to `false`.
    inherit: bool,
    /// Should start the counter disabled.
    ///
    /// Defaults to  `false`.
    start_disabled: bool,
    /// Count for kernel code.
    ///
    /// Defaults to `false`.
    collect_kernel: bool,
    /// Gather information on context switches.
    ///
    /// Defaults to `false`.
    gather_context_switches: bool,
    /// This corresponds to a sampled event that is accessed through a ring buffer.
    ///
    /// Defaults to false.
    is_sampled: bool,
    /// Size of requested ring buffer.
    ///
    /// Defaults to 128 * native page size..
    requested_size: usize,
}

impl Default for PerfEventBuilder {
    fn default() -> Self {
        PerfEventBuilder {
            name: String::new(),
            pid: 0,
            cpuid: -1,
            leader: -1,
            use_freq: false,
            freq_or_period: 1,
            inherit: false,
            start_disabled: false,
            collect_kernel: false,
            gather_context_switches: false,
            is_sampled: false,
            requested_size: (1 << 7) * *PAGE_SIZE,
        }
    }
}

macro_rules! builder_pattern {
    ($(#[$outer:meta])* $var_name: ident : $var_type: ty) => {
        builder_pattern!($(#[$outer])* $var_name => $var_name: $var_type);
    };
    ($(#[$outer:meta])* $var_name: ident : $var_type: ty = $val: expr) => {
        builder_pattern!($(#[$outer])* $var_name => $var_name: $var_type = $val);
    };
    ($(#[$outer:meta])* $name: ident => $var_name: ident : $var_type: ty) => {
        $(#[$outer])*
        pub fn $name(mut self, $var_name: $var_type) -> Self {
            self.$var_name = $var_name;
            self
        }
    };
    ($(#[$outer:meta])* $name: ident => $var_name: ident : $var_type: ty = $val: expr) => {
        $(#[$outer])*
        pub fn $name(mut self) -> Self {
            self.$var_name = $val;
            self
        }
    };
}

impl PerfEventBuilder {
    /// Get the maximum allowed sampling frequency of the system.
    fn _max_sampling_freq() -> Result<u64> {
        let data = std::fs::read_to_string("/proc/sys/kernel/perf_event_max_sample_rate")?;
        Ok(data.trim().parse::<u64>().unwrap())
    }

    /// Check capabilities of the current system and the configuration of the current builder.
    ///
    /// Returns `true` if the check resulted in failure.
    fn _check_capabilities(&self) -> Result<()> {
        let freq_check = if self.use_freq {
            self.freq_or_period > PerfEventBuilder::_max_sampling_freq()?
        } else {
            false
        };
        if (self.cpuid == -1 && self.inherit) || freq_check {
            Err(Error::KernelCapabilityError)
        } else {
            Ok(())
        }
    }

    /// Set the fields of an perf_event_attr based on this builder.
    fn _set_attr_config(&self, attr: &mut ffi::perf_event_attr) {
        use ffi::perf_event_read_format::*;
        use ffi::perf_event_sample_format::*;
        attr.size = std::mem::size_of::<ffi::perf_event_attr>()
            .try_into()
            .unwrap();
        if self.is_sampled {
            attr.read_format = PERF_FORMAT_ID as u64
                | PERF_FORMAT_TOTAL_TIME_RUNNING as u64
                | PERF_FORMAT_TOTAL_TIME_ENABLED as u64;
            attr.sample_type = PERF_SAMPLE_IP as u64
                | PERF_SAMPLE_TID as u64
                | PERF_SAMPLE_TIME as u64
                | PERF_SAMPLE_CPU as u64
                | PERF_SAMPLE_PERIOD as u64
                | PERF_SAMPLE_READ as u64;
            attr.__bindgen_anon_1.sample_period = self.freq_or_period;
            if self.use_freq {
                attr.set_freq(1);
            }
            attr.set_mmap(1);
            attr.set_mmap2(1);
            attr.set_mmap_data(1);
            if self.gather_context_switches {
                attr.set_context_switch(1);
            }
            attr.set_comm(1);
            attr.set_comm_exec(1);
        }
        attr.set_task(1);
        attr.set_sample_id_all(1);
        attr.set_exclude_callchain_user(1);
        attr.set_exclude_guest(1);
        attr.set_exclude_hv(1); // Maybe this should also be an option. Dont have hypervisors now.
        if self.start_disabled {
            attr.set_disabled(1);
        }
        if !self.collect_kernel {
            attr.set_exclude_kernel(1);
        }
        if self.inherit {
            attr.set_inherit(1);
        }
    }

    /// Internal implementation of open so as to not consume self.
    fn _open(&mut self, base_event_attr: Option<ffi::perf_event_attr>) -> Result<PerfEvent> {
        // Check validity
        self._check_capabilities()?;

        // Setup perf_event_attr
        let mut attr = if let Some(bea) = base_event_attr {
            bea
        } else {
            ffi::perf_event_attr::default()
        };
        self._set_attr_config(&mut attr);

        // Open file corresponding to perf_event_attr
        let fd = ffi::perf_event_open(
            &attr,
            self.pid,
            self.cpuid,
            self.leader,
            ffi::PERF_FLAG_FD_CLOEXEC as _,
        )?;

        debug!("Opened PerfEvent with attributes {:?}", attr);

        // Get ringbuffer corresponding to the fd
        let ring_buffer = if self.is_sampled {
            let req_space = self.requested_size;
            let log_num_pages = (1u32..26)
                .find(|x| (1 << *x) * *PAGE_SIZE >= req_space as _)
                .unwrap();
            let page_count = std::cmp::max(1 << log_num_pages, 16);
            Some(crate::perf::RingBuffer::new(fd, page_count as _)?)
        } else {
            None
        };

        // Ok... We are done
        Ok(PerfEvent {
            name: self.name.clone(),
            attr,
            file: unsafe { std::fs::File::from_raw_fd(fd) },
            ring_buffer,
        })
    }

    /// Generate the `PerfEvent` from this builder.
    ///
    /// If a `base_event_attr` is provided, all fields set in the builder will be overwritten.
    pub fn open(mut self, base_event_attr: Option<ffi::perf_event_attr>) -> Result<PerfEvent> {
        self._open(base_event_attr)
    }

    /// Generate a group of perf events from this builder.
    ///
    /// The first element of `base_event_attrs` is assumed to be the group leader.
    pub fn open_group(
        mut self,
        mut base_event_attrs: Vec<ffi::perf_event_attr>,
    ) -> Result<Vec<PerfEvent>> {
        // Create leader first
        let leader = self._open(Some(base_event_attrs.pop().unwrap()))?;
        // Create group using leader's fd
        self.leader = leader.file.as_raw_fd();
        let mut out = Vec::with_capacity(base_event_attrs.len() + 1);
        out.push(leader);
        for attr in base_event_attrs.iter_mut() {
            out.push(self._open(Some(*attr))?);
        }
        Ok(out)
    }

    builder_pattern!(
        /// Set the name of the event.
        name: String
    );

    builder_pattern!(
        /// Set process to be monitored.
        ///
        /// Set `0` for current process and `-1` for whole system.
        pid: libc::pid_t
    );

    builder_pattern!(
        /// Set CPU to be monitored.
        ///
        /// Set `-1` for whole system.
        cpuid: libc::c_int
    );

    builder_pattern!(
        /// Set group leader for this perf event
        leader: libc::c_int
    );

    builder_pattern!(
        /// Set collection period.
        set_period => freq_or_period: u64
    );

    builder_pattern!(
        /// Set collection frequency.
        set_frequency => freq_or_period: u64
    );

    builder_pattern!(
        /// Use frequency for this counter.
        use_frequency => use_freq: bool = true
    );

    builder_pattern!(
        /// Turns on kernel measurements.
        collect_kernel: bool = true
    );

    builder_pattern!(
        /// Inherit to children processes.
        inherit: bool = true
    );

    builder_pattern!(
        /// Start the counter disabled.
        start_disabled: bool = true
    );

    builder_pattern!(
        /// Gather data about context switches.
        gather_context_switches: bool = true
    );

    builder_pattern!(
        /// This performance counter will be sampled and accessed through the RingBuffer.
        enable_sampling => is_sampled: bool = true
    );

    builder_pattern!(
        /// Size requested for ring buffer.
        ///
        /// # Note
        /// This will be rounded of to the next multiple of native page size.
        requested_size: usize
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // The paranoid value needs to be set correctly for the other tests to pass
    #[test]
    fn test_kernel_paranoid_level() {
        let paranoid: i8 = std::fs::read_to_string("/proc/sys/kernel/perf_event_paranoid")
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(paranoid <= 2);
    }

    #[test]
    fn test_perf_read_fd() {
        // Create event
        let mut attr = ffi::perf_event_attr::default();
        attr.type_ = ffi::perf_type_id::PERF_TYPE_SOFTWARE as _;
        attr.config = ffi::perf_sw_ids::PERF_COUNT_SW_CPU_CLOCK as _;
        let evt = PerfEvent::build().start_disabled().open(Some(attr));
        assert!(evt.is_ok());
        let mut evt = evt.unwrap();
        assert!(evt.reset().is_ok());
        assert!(evt.enable().is_ok());
        assert!(evt.disable().is_ok());

        // Check error on wrong read method
        let evts = evt.read_samples();
        assert!(evts.is_empty());

        // Check value of count
        let count = evt.read_sync();
        assert!(count.is_ok());
        let count = count.unwrap();
        assert!(count.raw_value() > 0);
        assert!(count.scaled_value() > 0);
    }

    #[test]
    fn test_perf_read_ringbuffer() {
        // Create Event
        let mut attr = ffi::perf_event_attr::default();
        attr.type_ = ffi::perf_type_id::PERF_TYPE_HARDWARE as _;
        attr.config = ffi::perf_hw_id::PERF_COUNT_HW_CPU_CYCLES as _;
        let evt = PerfEvent::build()
            .start_disabled()
            .set_period(5)
            .enable_sampling()
            .open(Some(attr));
        assert!(evt.is_ok());
        let mut evt = evt.unwrap();

        for _ in 0..2 {
            // This checks the call to advance in the read_samples method.
            // Do some work
            assert!(evt.reset().is_ok());
            assert!(evt.enable().is_ok());
            let tmp: u32 = (0u32..100).filter(|x| x % 2 == 0).sum();
            println!("Val: {}", tmp);
            assert!(evt.disable().is_ok());

            // Check values of counters
            if let Some(ref rb) = evt.ring_buffer {
                assert!(rb.events_pending());
            }
            let counts = evt.read_samples();
            assert!(!counts.is_empty());
            for c in counts {
                assert!(c.value > 0);
            }
        }
    }
}
