use crate::perf::*;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::SeqCst;

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
pub struct MmappedRingBuffer {
    /// Metadata of the ring buffer.
    header: AtomicPtr<ffi::perf_event_mmap_page>,
    /// Size in bytes of the event records in the ring buffer.
    size: usize,
    /// Pointer to the begining of the event records.
    base: *mut u8,
    /// Extra memory in case the ring buffer overflows.
    extra: [u8; 256],
}

impl MmappedRingBuffer {
    /// Create a new mmaped buffer from perf event file descriptor `fd` and ring buffer size `npages`.
    pub fn new(fd: libc::c_int, npages: usize) -> crate::Result<Self> {
        assert_eq!(npages & (npages - 1), 0); // Check to see if npages is a power of 2
        unsafe {
            let pagesize = libc::sysconf(libc::_SC_PAGESIZE) as usize;
            let header = libc::mmap(
                std::ptr::null_mut(),
                pagesize * (npages + 1),
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            ) as *mut u8;
            if header != libc::MAP_FAILED as *mut u8 {
                Ok(MmappedRingBuffer {
                    header: std::mem::transmute(header),
                    size: pagesize * npages,
                    base: header.add(pagesize),
                    extra: [0u8; 256],
                })
            } else {
                Err(crate::ErrorKind::SystemError.into())
            }
        }
    }

    /// Access the events structure in the ring buffer.
    pub fn events(&mut self) -> Events {
        unsafe {
            let header = &mut *self.header.load(SeqCst);
            let head = header.data_head as isize;
            let tail = header.data_tail as isize;
            let size = self.size as isize;
            Events {
                header: &mut self.header,
                head: head as _,
                base: self.base,
                next: self.base.offset(tail % size),
                end: self.base.offset(head % size),
                limit: self.base.offset(size),
                extra: self.extra.as_mut_ptr(),
                marker: std::marker::PhantomData,
            }
        }
    }
}

/// All event records in the `MmappedRingBuffer`.
///
/// `'m` corresponds to the lifetime of the containing `MMapedRingBuffer`.
#[derive(Debug)]
pub struct Events<'m> {
    /// Metadata of the corresponding ring buffer.
    header: &'m mut AtomicPtr<ffi::perf_event_mmap_page>,
    /// Points to the head of the data section (unwrapped).
    head: *mut u8,
    /// Points to same location as `base` of containing `MmappedRingBuffer`.
    base: *mut u8,
    /// Points to last item read from userspace (wrapped).
    next: *mut u8,
    /// Points to last item written (wrapped).
    end: *mut u8,
    /// Points to last byte in the mmaped buffer.
    limit: *mut u8,
    /// Points to `extra` of the containing `MMapedRingBuffer`.
    extra: *mut u8,
    marker: std::marker::PhantomData<&'m u32>,
}

impl<'m> Events<'m> {
    /// Get the next `EventRecord`.
    #[allow(clippy::cast_ptr_alignment)]
    pub fn next_event(&mut self) -> Option<&'m EventRecord> {
        // Check if available
        if self.next == self.end {
            return None;
        }

        // Get new record
        let mut evt = unsafe { &*(self.next as *const EventRecord) };

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
                evt = &*(self.extra as *const EventRecord);
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

impl<'m> Drop for Events<'m> {
    fn drop(&mut self) {
        let header = self.header.load(SeqCst);
        unsafe {
            (*header).data_tail = self.head as u64;
        }
    }
}

/// Individual event record. Can be of type specified in `ffi::perf_event_type`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct EventRecord {
    /// Event header containing information about type of the event
    pub header: ffi::perf_event_header,
    /// First byte of the record.
    pub data: [u8; 0],
}

impl EventRecord {
    /// Get the raw data in an `EventRecord`.
    ///
    /// The memory layout of the raw data is given by `T`.
    pub unsafe fn get_data<T>(&self) -> &T {
        assert!(std::mem::size_of::<T>() <= self.header.size as usize);
        &*(self.data.as_ptr() as *const T)
    }
}
