//! Utilities to read sampled events from memory mapped ring buffer.

use crate::perf::*;
use crate::Result;
use nix::libc;
use nix::sys::mman;
use std::convert::TryInto;

/// Userspace wrapper for the sampled/mmaped perf events.
///
/// Memory layout:
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
    /// The size of the allocation made using `mmap`.
    total_alloc_size: usize,
    /// Size in bytes of the event records in the ring buffer.
    size: usize,
    /// Pointer to the beginning of the event records.
    base: *mut u8,
    /// Extra memory in case the ring buffer overflows.
    extra: [u64; 32], // Need 256 bytes defined like this to use the derive Debug macro
}

impl RingBuffer {
    /// Create a new mmaped buffer from perf event file descriptor `fd` and ring buffer size `npages`.
    ///
    /// # Panics
    /// `npages` must be a power of 2. The call will panic otherwise.
    pub fn new(fd: libc::c_int, npages: usize) -> Result<Self> {
        assert_eq!(npages & (npages - 1), 0); // Check to see if npages is a power of 2
        let pagesize: usize = nix::unistd::sysconf(nix::unistd::SysconfVar::PAGE_SIZE)?
            .unwrap()
            .try_into()
            .unwrap();
        let rb = unsafe {
            let header = mman::mmap(
                std::ptr::null_mut(),
                pagesize * (npages + 1),
                mman::ProtFlags::PROT_READ | mman::ProtFlags::PROT_WRITE,
                mman::MapFlags::MAP_SHARED,
                fd,
                0,
            )? as *mut ffi::perf_event_mmap_page;
            RingBuffer {
                header,
                total_alloc_size: pagesize * (npages + 1),
                size: pagesize * npages,
                base: header.add(1) as *mut u8,
                extra: [0u64; 32],
            }
        };
        Ok(rb)
    }

    /// Access the events structure in the ring buffer.
    pub fn events(&mut self) -> RingBufferEvents {
        let header = unsafe { &mut *(self.header) };
        let head = header.data_head as isize;
        let tail = header.data_tail as isize;
        let size = self.size as isize;
        unsafe {
            RingBufferEvents {
                header: &mut *(self.header),
                head: head as _,
                base: self.base,
                next: self.base.offset(tail % size),
                end: self.base.offset(head % size),
                limit: self.base.offset(size),
                extra: self.extra.as_mut_ptr() as *mut u8,
                marker: std::marker::PhantomData,
            }
        }
    }
}

impl Drop for RingBuffer {
    fn drop(&mut self) {
        unsafe {
            let _ = mman::munmap(self.header as *mut std::ffi::c_void, self.total_alloc_size);
        }
    }
}

/// All event records in a `RingBuffer`.
///
/// `'m` corresponds to the lifetime of the containing `RingBuffer`.
#[derive(Debug)]
pub struct RingBufferEvents<'m> {
    /// Metadata of the corresponding ring buffer.
    header: &'m mut ffi::perf_event_mmap_page,
    /// Points to the head of the data section (unwrapped).
    head: *mut u8,
    /// Points to same location as `base` of containing `RingBuffer`.
    base: *mut u8,
    /// Points to last item read from userspace (wrapped).
    next: *mut u8,
    /// Points to last item written (wrapped).
    end: *mut u8,
    /// Points to last byte in the mmaped buffer.
    limit: *mut u8,
    /// Points to `extra` of the containing `RingBuffer`.
    extra: *mut u8,
    marker: std::marker::PhantomData<&'m u32>,
}

impl<'m> RingBufferEvents<'m> {
    /// Get the next `EventRecord`.
    #[allow(clippy::cast_ptr_alignment)]
    pub fn next_event(&mut self) -> Option<&'m RawEvent> {
        // Check if available
        if self.next == self.end {
            return None;
        }

        // Get new record
        let mut evt = unsafe { &*(self.next as *const RawEvent) };

        // Update next
        let size = evt.header.size as isize;
        let next = unsafe { self.next.offset(size) };
        let limit = self.limit;
        self.next = if next > limit {
            unsafe {
                let len = limit.offset(-(self.next as isize)) as isize;
                let (p0, l0) = (self.extra, len);
                let (p1, l1) = (self.extra.offset(len), size - len);
                std::ptr::copy_nonoverlapping(self.next, p0, l0 as _);
                std::ptr::copy_nonoverlapping(self.base, p1, l1 as _);
                evt = &*(self.extra as *const RawEvent);
                self.base.offset(l1)
            }
        } else if next == limit {
            self.base
        } else {
            next
        };

        Some(evt)
    }
}

impl<'m> Drop for RingBufferEvents<'m> {
    fn drop(&mut self) {
        self.header.data_tail = self.head as u64;
    }
}

/// Individual event record. Can be of type specified in `ffi::perf_event_type`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RawEvent {
    /// Event header containing information about type of the event
    pub header: ffi::perf_event_header,
    /// First byte of the record.
    pub data: [u8; 0],
}

impl RawEvent {
    /// Get the raw data in an `EventRecord`.
    ///
    /// The memory layout of the raw data is given by `T`.
    pub unsafe fn get_data<T>(&self) -> &T {
        assert!(std::mem::size_of::<T>() <= self.header.size as usize);
        &*(self.data.as_ptr() as *const T)
    }
}
