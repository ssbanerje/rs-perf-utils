//! Utilities for creating/opening perf events.

use crate::perf::ffi;
use crate::{Error, Result};
use log::info;
use nix::libc;
use std::convert::TryInto;

/// A schedulable and readable performance counter.
///
/// Represents a readable perf event which can be used to collect data directly from the kernel,
/// through the memory mapped ring buffer, or through direct read from hardware.
#[derive(Debug)]
pub struct PerfEvent {
    /// Attributes corresponding to this event.
    pub attr: ffi::perf_event_attr,
    /// File descriptor of underlying perf event.
    pub fd: libc::c_int,
    /// Ring buffer corresponding to underlying perf event.
    pub ring_buffer: Option<crate::perf::RingBuffer>,
}

impl Drop for PerfEvent {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

impl PerfEvent {
    /// Construct a new perf event using the associated builder,
    pub fn build() -> PerfEventBuilder {
        PerfEventBuilder::default()
    }

    /// Enable counting for event.
    pub fn enable(&self) -> Result<()> {
        unsafe {
            ffi::perf_event_ioc_enable(self.fd)?;
        }
        Ok(())
    }

    /// Disable counting for event.
    pub fn disable(&self) -> Result<()> {
        unsafe {
            ffi::perf_event_ioc_disable(self.fd)?;
        }
        Ok(())
    }

    /// Reset counting for event.
    pub fn reset(&self) -> Result<()> {
        unsafe {
            ffi::perf_event_ioc_reset(self.fd)?;
        }
        Ok(())
    }

    /// Poll event for new samples.
    pub fn poll(&self, timeout: libc::c_int) -> Result<nix::poll::PollFlags> {
        let mut pollfd = [nix::poll::PollFd::new(
            self.fd,
            nix::poll::PollFlags::POLLIN | nix::poll::PollFlags::POLLHUP,
        )];
        nix::poll::poll(&mut pollfd, timeout)?;
        match pollfd[0].revents() {
            Some(x) => Ok(x),
            _ => unreachable!(),
        }
    }

    /// Check if target process of this event has exited.
    pub fn is_closed(&self) -> Result<bool> {
        let poll = self.poll(0)?;
        Ok(poll.intersects(nix::poll::PollFlags::POLLHUP))
    }

    /// Write modifications to `self.attr` to the kernel.
    pub fn modify_event_attributes(&mut self) -> Result<()> {
        unsafe {
            ffi::perf_event_ioc_modify_attributes(
                self.fd,
                &mut self.attr as *mut ffi::perf_event_attr,
            )?;
        }
        Ok(())
    }

    /// Modify the frequency or period at which this event is sampled.
    ///
    /// By default the event is frequency based. To switch to period modify the event's `attr` field
    /// and write the modifications to the kernel using `modify_event_attributes`.
    pub fn modify_frequency_period(&mut self, freq: u64) -> Result<()> {
        unsafe {
            ffi::perf_event_ioc_period(self.fd, freq)?;
        }
        Ok(())
    }
}

/// Trait allowing for performance counter data to be directly through the Kernel.
pub trait OsReadable {
    /// Read value of performance counter from it's file descriptor.
    fn read_fd(&self) -> Result<u64>;
}

impl OsReadable for PerfEvent {
    fn read_fd(&self) -> Result<u64> {
        if self.ring_buffer.is_none() {
            let mut bytes = [0u8; 8];
            nix::unistd::read(self.fd, &mut bytes)?;
            Ok(u64::from_ne_bytes(bytes))
        } else {
            Err(Error::WrongReadMethod)
        }
    }
}

/// Trait allowing for performance counter data to be directly read from hardware.
pub trait HardwareReadable {
    /// Function to read performance counter data.
    fn read_hw(&self) -> Result<u64>;
}

/// Helper struct to build a `PerfEvent` object.
#[derive(Debug)]
pub struct PerfEventBuilder {
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
    /// Sampling frequency.
    ///
    /// Defaults to `0`.
    freq: u64,
    /// Size of stack to be dumped with samples.
    ///
    /// Defaults to `0`.
    stack_size: u32,
    /// Mask corresponding to user registers that will be dumped with samples.
    ///
    /// Defaults to `0`.
    reg_mask: u64,
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
}

impl Default for PerfEventBuilder {
    fn default() -> Self {
        PerfEventBuilder {
            pid: 0,
            cpuid: -1,
            leader: -1,
            freq: 0,
            stack_size: 0,
            reg_mask: 0,
            inherit: false,
            start_disabled: false,
            collect_kernel: false,
            gather_context_switches: false,
            is_sampled: false,
        }
    }
}

macro_rules! builder_pattern {
    ($(#[$outer:meta])* $var_name: ident : $var_type: ty) => {
        builder_pattern!($(#[$outer])* $var_name => $var_name: $var_type);
    };
    ($(#[$outer:meta])* $name: ident => $var_name: ident : $var_type: ty) => {
        $(#[$outer])*
        pub fn $name(mut self, $var_name: $var_type) -> Self {
            self.$var_name = $var_name;
            self
        }
    };
}

macro_rules! builder_pattern_bool {
    ($(#[$outer:meta])* $var_name: ident) => {
        builder_pattern_bool!($(#[$outer])* $var_name => $var_name);
    };
    ($(#[$outer:meta])* $name: ident => $var_name: ident) => {
        $(#[$outer])*
        pub fn $name(mut self) -> Self {
            self.$var_name = true;
            self
        }
    };
}

impl PerfEventBuilder {
    /// Check capabilities of the current system and the configuration of the current builder.
    fn _check_capabilities(&self) -> Result<bool> {
        // Check max sampling rate from the kernel
        let data = std::fs::read_to_string("/proc/sys/kernel/perf_event_max_sample_rate")?;
        let max_sampling_rate = data.trim().parse::<u64>().unwrap();

        // Check perf_builder
        Ok((self.cpuid == -1 && self.inherit)
            || self.freq > max_sampling_rate
            || self.stack_size > 63 * 1024)
    }

    /// Set the fields of an perf_event_attr based on this builder.
    fn _set_attr_config(&self, attr: &mut ffi::perf_event_attr) {
        use ffi::perf_event_sample_format::*;
        attr.size = std::mem::size_of::<ffi::perf_event_attr>()
            .try_into()
            .unwrap();
        if self.is_sampled {
            attr.sample_type = PERF_SAMPLE_IP as u64
                | PERF_SAMPLE_TID as u64
                | PERF_SAMPLE_TIME as u64
                | PERF_SAMPLE_CALLCHAIN as u64
                | PERF_SAMPLE_CPU as u64
                | PERF_SAMPLE_PERIOD as u64;
            if self.reg_mask != 0 {
                attr.sample_type |= PERF_SAMPLE_REGS_USER as u64;
            }
            if self.stack_size != 0 {
                attr.sample_type |= PERF_SAMPLE_STACK_USER as u64;
            }
            attr.sample_regs_user = self.reg_mask;
            attr.sample_stack_user = self.stack_size;
            attr.__bindgen_anon_1.sample_freq = self.freq;
            attr.set_freq(1);
            attr.set_mmap(1);
            attr.set_mmap2(1);
            attr.set_mmap_data(1);
            if self.gather_context_switches {
                attr.set_context_switch(1);
            }
            attr.set_comm(1);
        }
        attr.set_task(1);
        attr.set_exclude_callchain_user(1);
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
    fn _open(&self, base_event_attr: Option<ffi::perf_event_attr>) -> Result<PerfEvent> {
        info!("Opening perf event {:?}", &self);

        // Check validity
        if self._check_capabilities()? {
            return Err(Error::PerfNotCapable);
        }

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

        // Get ringbuffer corresponding to the fd
        let ring_buffer = if self.is_sampled {
            let page_size: u32 = nix::unistd::sysconf(nix::unistd::SysconfVar::PAGE_SIZE)?
                .unwrap()
                .try_into()
                .unwrap();
            let req_space = std::cmp::max(page_size, self.stack_size);
            let log_num_pages = (1u32..26)
                .find(|x| (1 << *x) * page_size >= req_space)
                .unwrap();
            let page_count = std::cmp::max(1 << log_num_pages, 16);
            Some(crate::perf::RingBuffer::new(fd, page_count as _)?)
        } else {
            None
        };

        // Ok... We are done
        Ok(PerfEvent {
            attr,
            fd,
            ring_buffer,
        })
    }

    /// Generate the `PerfEvent` from this builder.
    ///
    /// If a `base_event_attr` is provided, all fields set in the builder will be overwritten.
    pub fn open(&self, base_event_attr: Option<ffi::perf_event_attr>) -> Result<PerfEvent> {
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
        self.leader = leader.fd;
        let mut out = Vec::with_capacity(base_event_attrs.len() + 1);
        out.push(leader);
        for attr in base_event_attrs.iter_mut() {
            out.push(self._open(Some(*attr))?);
        }
        Ok(out)
    }

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
        /// Set collection frequency.
        frequency => freq: u64
    );

    builder_pattern!(
        /// User registers to be dumped with every sample.
        dump_user_regs => reg_mask: u64
    );

    builder_pattern!(
        /// Size of user stack to be dumped on every sample.
        dump_stack_size => stack_size: u32
    );

    builder_pattern_bool!(
        /// Turns on kernel measurements.
        collect_kernel
    );

    builder_pattern_bool!(
        /// Inherit to children processes.
        inherit
    );

    builder_pattern_bool!(
        /// Start the counter disabled.
        start_disabled
    );

    builder_pattern_bool!(
        /// Gather data about context switches.
        gather_context_switches
    );

    builder_pattern_bool!(
        /// This performance counter will be sampled and accessed through the RingBuffer.
        use_ring_buffer => is_sampled
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_event_open() {
        let paranoid: i8 = std::fs::read_to_string("/proc/sys/kernel/perf_event_paranoid")
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(paranoid <= 2);
        let mut attr = ffi::perf_event_attr::default();
        attr.type_ = ffi::perf_type_id::PERF_TYPE_SOFTWARE as _;
        attr.config = ffi::perf_sw_ids::PERF_COUNT_SW_TASK_CLOCK as _;
        let evt = PerfEvent::build().start_disabled().open(Some(attr));
        assert!(evt.is_ok());
        let evt = evt.unwrap();

        assert!(evt.reset().is_ok());
        assert!(evt.enable().is_ok());
        assert!(evt.disable().is_ok());

        let count = evt.read_fd();
        assert!(count.is_ok());
        let count = count.unwrap();
        assert!(count > 0);
    }
}
