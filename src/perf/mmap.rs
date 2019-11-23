//! Utilities to read sampled events from memory mapped ring buffer.

use crate::perf::*;
use crate::Result;
use byteorder::{NativeEndian, ReadBytesExt};
use derive_more::{Index, IndexMut};
use lazy_static::lazy_static;
use log::debug;
use nix::libc;
use nix::sys::mman;
use std::convert::TryInto;

lazy_static! {
    /// Size of a single memory page on the machine.
    pub static ref PAGE_SIZE: usize = {
        nix::unistd::sysconf(nix::unistd::SysconfVar::PAGE_SIZE)
            .unwrap()
            .unwrap()
            .try_into()
            .unwrap()
    };
}

/// Internal implementation of the `read_data_head` function.
fn _read_data_head(header: *const ffi::perf_event_mmap_page) -> u64 {
    let header = unsafe { &*header };
    let head = volatile!(header.data_head);
    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
    head
}

/// Internal implementation og the `write_data_tail` function.
fn _write_data_tail(header: *mut ffi::perf_event_mmap_page, value: u64) {
    let header = unsafe { &mut *header };
    std::sync::atomic::fence(std::sync::atomic::Ordering::AcqRel);
    volatile!(header.data_tail, value);
}

/// Userspace wrapper for the sampled/mmaped perf events.
///
/// # Memory layout
/// ```text
/// ┌───── header ─────┐  ▲
/// │                  │  │
/// │ perf_event_mmap  │ pagesize
/// │      _page       │  │
/// │                  │  ▼
/// ├─────  base  ─────┤  ▲
/// │                  │  │
/// │                  │ size
/// │      Events      │  │
/// │                  │  │
/// └──────────────────┘  ▼
/// ```
#[derive(Debug)]
pub struct RingBuffer {
    /// Metadata of the ring buffer.
    pub header: *mut ffi::perf_event_mmap_page,
    /// Pointer to the beginning of the event records.
    pub base: *mut u8,
    /// Size in bytes of the event records in the ring buffer.
    pub size: usize,
    /// Total number of bytes read from the buffer.
    ///
    /// This is used to set `data_tail`.
    total_bytes_read: u64,
}

impl RingBuffer {
    /// Create a new mmaped buffer from perf event file descriptor `fd` and ring buffer size `npages`.
    ///
    /// # Panics
    /// `npages` must be a power of 2. The call will panic otherwise.
    pub fn new(fd: libc::c_int, npages: usize) -> Result<Self> {
        assert_eq!(npages & (npages - 1), 0); // Check to see if npages is a power of 2
        let header = unsafe {
            mman::mmap(
                std::ptr::null_mut(),
                *PAGE_SIZE * (npages + 1),
                mman::ProtFlags::PROT_READ | mman::ProtFlags::PROT_WRITE,
                mman::MapFlags::MAP_SHARED,
                fd,
                0,
            )? as *mut ffi::perf_event_mmap_page
        };
        let rb = RingBuffer {
            header,
            base: unsafe { (header as *mut u8).add(*PAGE_SIZE) },
            size: *PAGE_SIZE * npages,
            total_bytes_read: 0,
        };
        Ok(rb)
    }

    /// Get an iterator over the events that have been added to the buffer from the kernel.
    ///
    /// The iterator will not update as new events are added, it only contains elements present
    /// in the ring buffer at the time of the call.
    ///
    /// The iterator will will not advance the tail of the buffer. Doing so will require explicit
    /// calls to `advance`.
    pub fn events(&mut self) -> RingBufferIter {
        RingBufferIter::new(self)
    }

    /// Notify the kernel that `num` elements of samples has been read from the `RingBuffer`.
    ///
    /// The call will clear the buffer if `None` is passed to the `num` field.
    pub fn advance(&mut self, num: Option<usize>) {
        // Get the position of the buffer to advance data_tail
        let header = self.header;
        let mut iter = self.events();
        let bytes_read = if let Some(n) = num {
            let _ = iter.nth(n);
            iter.bytes_read
        } else {
            let _ = iter.try_fold(0, |_, _| Some(0)); // goto last entry
            iter.bytes_read
        };
        self.total_bytes_read += bytes_read;

        // Write value to data_tail
        _write_data_tail(header, self.total_bytes_read);
    }

    /// Checks whether there are pending events.
    ///
    /// Returns `true` if there are pending events.
    ///
    /// Subsequent calls to `event_pending` and `events` are not guaranteed to be perform an atomic
    /// check (i.e., events can be enqueued into the buffer in between calls).
    pub fn events_pending(&self) -> bool {
        let head = _read_data_head(self.header);
        let tail = unsafe { &*self.header }.data_tail;
        (tail % self.size as u64) != (head % self.size as u64)
    }
}

impl Drop for RingBuffer {
    fn drop(&mut self) {
        // Consume all entries (Not sure if the kernel requires this... Probably not)
        self.advance(None);
        // Unmap buffer
        let _ =
            unsafe { mman::munmap(self.header as *mut std::ffi::c_void, self.size + *PAGE_SIZE) };
    }
}

unsafe impl Send for RingBuffer {}

/// Iterator over records in a `RingBuffer`.
///
/// `'m` corresponds to the lifetime of the containing `RingBuffer`.
///
/// The `RingBuffer` also allows directly accessing the bytes of the mapped buffer using the the
/// `index` and `index_mut` functions.
///
/// # Memory layout
/// ```text
///      +--data+------+
///      |             |
/// next |             |
///     +------>       |
///      |             |
/// end  |             |
///     +------>       |
///      |             |
///      +-------------+
/// ```
#[derive(Index, IndexMut)]
pub struct RingBufferIter<'m> {
    /// Pointer to the start of the data section of the `RingBuffer`.
    #[index]
    #[index_mut]
    data: &'m mut [u8],
    /// Byte index of the last item read from userspace (wrapped).
    next_idx: u64,
    /// Byte index of the last item read from kernel (wrapped).
    end_idx: u64,
    /// Total bytes read by the iterator.
    bytes_read: u64,
    /// Extra memory to store an event that has been wrapped around the end of the `RingBuffer`.
    extra: [u8; 256],
}

impl<'m> RingBufferIter<'m> {
    /// Create a new iterator for a `RingBuffer`.
    pub(crate) fn new(buf: &'m mut RingBuffer) -> Self {
        let data_head = _read_data_head(buf.header);
        let data_tail = unsafe { (*buf.header).data_tail as u64 };
        RingBufferIter {
            data: unsafe { std::slice::from_raw_parts_mut(buf.base, buf.size) },
            next_idx: data_tail % buf.size as u64,
            end_idx: data_head % buf.size as u64,
            bytes_read: 0,
            extra: [0u8; 256],
        }
    }

    /// Get the `RawRecord` at position `self.next`.
    #[allow(clippy::cast_ptr_alignment)]
    #[inline(always)]
    fn _get_next_record(&self) -> &'m RawRecord {
        let ptr = &self.data[self.next_idx as usize] as *const u8 as *const RawRecord;
        unsafe { &*ptr }
    }

    /// Get the `RawRecord` stored in `self.extra`.
    #[allow(clippy::cast_ptr_alignment)]
    #[inline(always)]
    fn _get_record_at_extra(&self) -> &'m RawRecord {
        unsafe { &*(&self.extra as *const u8 as *const RawRecord) }
    }
}

impl<'m> Iterator for RingBufferIter<'m> {
    type Item = &'m RawRecord;

    fn next(&mut self) -> Option<Self::Item> {
        // Check if available
        if self.next_idx == self.end_idx {
            return None;
        }

        // Get new record
        let mut evt = self._get_next_record();

        // Update next
        let next = self.next_idx + evt.header.size as u64;
        let limit = self.data.len() as u64;
        self.next_idx = if next > limit {
            // Copy data from end of data to extra
            let num_at_end = limit - self.next_idx;
            let src_range = (self.next_idx as usize)..(limit as usize);
            let dst_range = 0..(num_at_end as usize);
            self.extra[dst_range].copy_from_slice(&self.data[src_range]);

            // Copy data from begining of data to extra
            let num_at_beg = evt.header.size as u64 - num_at_end;
            let src_range = 0..(num_at_beg as usize);
            let dst_range = (num_at_end as usize)..((num_at_end + num_at_beg) as usize);
            self.extra[dst_range].copy_from_slice(&self.data[src_range]);

            // Set evt to point to the extra buffer
            evt = self._get_record_at_extra();

            // Done
            num_at_beg
        } else if next == limit {
            0
        } else {
            next
        };

        // Maintain total bytes read
        self.bytes_read += evt.header.size as u64;

        Some(evt)
    }
}

impl std::fmt::Debug for RingBufferIter<'_> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "RingBufferIter ")?;
        fmt.debug_map()
            .entry(&"next", &self.next_idx)
            .entry(&"end", &self.end_idx)
            .entry(&"bytes_read", &self.bytes_read)
            .entry(&"data.ptr", &self.data.as_ptr())
            .entry(&"data.len", &self.data.len())
            .entry(&"extra.ptr", &self.extra.as_ptr())
            .finish()
    }
}

unsafe impl Send for RingBufferIter<'_> {}

/// Individual record in a `RingBuffer`.
#[repr(C)]
#[derive(Debug)]
pub struct RawRecord {
    /// Event header containing information about type of the event.
    pub header: ffi::perf_event_header,
    /// First byte of the record after the header.
    pub data: [u8; 0],
}

impl RawRecord {
    /// Check if this record is measurement sample.
    pub fn is_sample(&self) -> bool {
        self.header.type_ == ffi::perf_event_type::PERF_RECORD_SAMPLE as u32
    }

    /// Parse the raw data in this record to construct a `ParsedRingBufferRecord`.
    ///
    /// Only call this on the events of interest as this function will allocate new memory and
    /// memcopy each event.
    ///
    /// # Note
    /// The implementation of this function is closely tied to that of the `PerfEventBuilder` with
    /// only configurations supported there being implemented here.
    pub fn parse(&self) -> Result<ParsedRecord> {
        let raw_data = unsafe {
            std::slice::from_raw_parts(
                self.data.as_ptr(),
                self.header.size as usize - std::mem::size_of::<ffi::perf_event_header>(),
            )
        };

        debug!(
            "Parsing RawRecord {:?} with data\n{}",
            self.header,
            crate::util::hexdump(raw_data)
        );

        let mut ptr = std::io::Cursor::new(raw_data);
        let res = match self.header.type_.into() {
            ffi::perf_event_type::PERF_RECORD_SWITCH => {
                let is_out = (self.header.misc & ffi::PERF_RECORD_MISC_SWITCH_OUT as u16) != 0;
                let is_preempt =
                    (self.header.misc & ffi::PERF_RECORD_MISC_SWITCH_OUT_PREEMPT as u16) != 0;
                ParsedRecord::ContextSwitch(if is_out {
                    if is_preempt {
                        ContextSwitchRecord::SwitchOutRunning
                    } else {
                        ContextSwitchRecord::SwitchOutIdle
                    }
                } else {
                    ContextSwitchRecord::SwitchIn
                })
            }

            ffi::perf_event_type::PERF_RECORD_EXIT => ParsedRecord::Exit(ProcessRecord {
                pid: ptr.read_u32::<NativeEndian>()?,
                ppid: ptr.read_u32::<NativeEndian>()?,
                tid: ptr.read_u32::<NativeEndian>()?,
                ptid: ptr.read_u32::<NativeEndian>()?,
                time: ptr.read_u64::<NativeEndian>()?,
            }),

            ffi::perf_event_type::PERF_RECORD_FORK => ParsedRecord::Fork(ProcessRecord {
                pid: ptr.read_u32::<NativeEndian>()?,
                ppid: ptr.read_u32::<NativeEndian>()?,
                tid: ptr.read_u32::<NativeEndian>()?,
                ptid: ptr.read_u32::<NativeEndian>()?,
                time: ptr.read_u64::<NativeEndian>()?,
            }),

            ffi::perf_event_type::PERF_RECORD_THROTTLE => ParsedRecord::Throttle(ThrottleRecord {
                time: ptr.read_u64::<NativeEndian>()?,
                id: ptr.read_u64::<NativeEndian>()?,
                stream_id: ptr.read_u64::<NativeEndian>()?,
            }),

            ffi::perf_event_type::PERF_RECORD_UNTHROTTLE => {
                ParsedRecord::UnThrottle(ThrottleRecord {
                    time: ptr.read_u64::<NativeEndian>()?,
                    id: ptr.read_u64::<NativeEndian>()?,
                    stream_id: ptr.read_u64::<NativeEndian>()?,
                })
            }

            ffi::perf_event_type::PERF_RECORD_LOST => ParsedRecord::Lost(LostRecord {
                id: ptr.read_u64::<NativeEndian>()?,
                num: ptr.read_u64::<NativeEndian>()?,
            }),

            ffi::perf_event_type::PERF_RECORD_COMM => ParsedRecord::Comm(CommRecord {
                pid: ptr.read_u32::<NativeEndian>()?,
                tid: ptr.read_u32::<NativeEndian>()?,
                comm: {
                    let raw_comm = &raw_data[ptr.position() as usize..];
                    let filter_comm = &raw_comm[0..raw_comm
                        .iter()
                        .position(|&byte| byte == 0)
                        .unwrap_or_else(|| raw_comm.len())];
                    std::str::from_utf8(filter_comm)?.into()
                },
            }),

            ffi::perf_event_type::PERF_RECORD_MMAP2 => ParsedRecord::Mmap2(Mmap2Record {
                pid: ptr.read_u32::<NativeEndian>()?,
                tid: ptr.read_u32::<NativeEndian>()?,
                address: ptr.read_u64::<NativeEndian>()?,
                length: ptr.read_u64::<NativeEndian>()?,
                page_offset: ptr.read_u64::<NativeEndian>()?,
                major: ptr.read_u32::<NativeEndian>()?,
                minor: ptr.read_u32::<NativeEndian>()?,
                inode: ptr.read_u64::<NativeEndian>()?,
                inode_generation: ptr.read_u64::<NativeEndian>()?,
                protection: ptr.read_u32::<NativeEndian>()?,
                flags: ptr.read_u32::<NativeEndian>()?,
                filename: {
                    let raw_name = &raw_data[ptr.position() as usize..];
                    let filtered_name = &raw_name[0..raw_name
                        .iter()
                        .position(|&byte| byte == 0)
                        .unwrap_or_else(|| raw_name.len())];
                    std::str::from_utf8(filtered_name)?.into()
                },
            }),

            ffi::perf_event_type::PERF_RECORD_SAMPLE => ParsedRecord::Sample(SampleRecord {
                ip: ptr.read_u64::<NativeEndian>()?,
                pid: ptr.read_u32::<NativeEndian>()?,
                tid: ptr.read_u32::<NativeEndian>()?,
                time: ptr.read_u64::<NativeEndian>()?,
                cpu: ptr.read_u32::<NativeEndian>()?,
                period: {
                    let _ = ptr.read_u32::<NativeEndian>()?; // Reserved field res
                    ptr.read_u64::<NativeEndian>()?
                },
                value: PerfEventValue::from_cursor(&mut ptr)?,
            }),

            _ => ParsedRecord::UnknownEvent,
        };
        Ok(res)
    }
}

/// Ring buffer records corresponding to context switches.
#[derive(Debug)]
pub enum ContextSwitchRecord {
    /// Process switched in.
    SwitchIn,
    /// Process switched out when idle.
    SwitchOutIdle,
    /// Process switched out when running.
    SwitchOutRunning,
}

/// Ring buffer records corresponding to process forks and exits.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct ProcessRecord {
    pub pid: u32,
    pub ppid: u32,
    pub tid: u32,
    pub ptid: u32,
    pub time: u64,
}

/// Ring buffer records corresponding to throttle and unthrottle events.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct ThrottleRecord {
    pub time: u64,
    pub id: u64,
    pub stream_id: u64,
}

/// Ring buffer records corresponding to lost samples.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct LostRecord {
    pub id: u64,
    pub num: u64,
}

/// Ring buffer records corresponding to changes in process names.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct CommRecord {
    pub pid: u32,
    pub tid: u32,
    pub comm: String,
}

/// Ring buffer records with information about `mmap` calls.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct Mmap2Record {
    pub pid: u32,
    pub tid: u32,
    pub address: u64,
    pub length: u64,
    pub page_offset: u64,
    pub major: u32,
    pub minor: u32,
    pub inode: u64,
    pub inode_generation: u64,
    pub protection: u32,
    pub flags: u32,
    pub filename: String,
}

/// Ring buffer records corresponding to a sampled perf event.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct SampleRecord {
    pub ip: u64,
    pub pid: u32,
    pub tid: u32,
    pub time: u64,
    pub cpu: u32,
    pub period: u64,
    pub value: crate::perf::PerfEventValue,
}

/// Ring buffer records with parsed fields.
#[derive(Debug)]
pub enum ParsedRecord {
    /// Record corresponding to `PERF_RECORD_SWITCH`.
    ContextSwitch(ContextSwitchRecord),
    /// Record corresponding to `PERF_RECORD_EXIT`.
    Exit(ProcessRecord),
    /// Record corresponding to `PERF_RECORD_FORK`.
    Fork(ProcessRecord),
    /// Record corresponding to `PERF_RECORD_THROTTLE`.
    Throttle(ThrottleRecord),
    /// Record corresponding to `PERF_RECORD_UNTHROTTLE`.
    UnThrottle(ThrottleRecord),
    /// Record corresponding to `PERF_RECORD_LOST`.
    Lost(LostRecord),
    /// Record corresponding to `PERF_RECORD_COMM`.
    Comm(CommRecord),
    /// Record corresponding to `PERF_RECORD_MMAP2`.
    Mmap2(Mmap2Record),
    /// Record corresponding to `PERF_RECORD_SAMPLE`.
    Sample(SampleRecord),
    /// Record corresponding to all unimplemented `PERF_RECORD_*` types.
    UnknownEvent,
}
