//! Utilities specific to the x86 architecture.

#![allow(non_snake_case)]

use crate::perf::*;

extern "C" {
    fn rdpmc(counter: u32) -> u64;
}

/// Allows accessing CPU performance counters from ring 3 using the `perf_events` subsystem.
#[derive(Debug)]
pub struct RDPMC {
    /// File descriptor to performance counter.
    fd: i32,
    /// Memory mapped structure storing the performance counter.
    buf: *mut ffi::perf_event_mmap_page,
}

impl RDPMC {
    /// Initialize the performance counter using its raw even descriptor.
    pub fn open_raw_desc(
        counter: u64,
        leader: Option<&RDPMC>,
        pid: libc::pid_t,
        cpuid: libc::c_int,
    ) -> crate::Result<RDPMC> {
        let mut attr = ffi::perf_event_attr::default();
        attr.type_ = (if counter > 10 {
            ffi::perf_type_id::PERF_TYPE_RAW
        } else {
            ffi::perf_type_id::PERF_TYPE_HARDWARE
        }) as _;
        attr.size = std::mem::size_of_val(&attr) as _;
        attr.config = counter;
        attr.sample_type = ffi::perf_event_sample_format::PERF_SAMPLE_READ as _;
        attr.set_exclude_kernel(1);
        attr.set_exclude_hv(1);
        RDPMC::open_perf_attr(&attr, leader, pid, cpuid)
    }

    /// Initialize the performance counter using its a `PerfEventAttr`.
    pub fn open_perf_attr(
        attr: &ffi::perf_event_attr,
        leader: Option<&RDPMC>,
        pid: libc::pid_t,
        cpuid: libc::c_int,
    ) -> crate::Result<RDPMC> {
        let mut new_ctr = RDPMC {
            fd: perf_event_open(
                attr,
                pid,
                cpuid,
                match leader {
                    Some(x) => x.fd,
                    None => -1,
                },
                0,
            )?,
            buf: std::ptr::null_mut(),
        };
        unsafe {
            new_ctr.buf = libc::mmap(
                std::ptr::null_mut(),
                libc::sysconf(libc::_SC_PAGESIZE) as usize,
                libc::PROT_READ,
                libc::MAP_SHARED,
                new_ctr.fd,
                0,
            ) as *mut ffi::perf_event_mmap_page;
        }
        Ok(new_ctr)
    }

    /// Read the performance counter.
    pub fn read(&mut self) -> u64 {
        let mut val: u64;
        let mut offset: i64;
        let mut seq: u32;
        let mut idx: u32;
        loop {
            unsafe {
                seq = (*(self.buf)).lock;
                std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);

                idx = (*(self.buf)).index;
                offset = (*(self.buf)).offset;
                if idx == 0 {
                    val = 0;
                    break;
                }
                val = rdpmc(idx - 1);
                std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);

                if seq != (*(self.buf)).lock {
                    break;
                }
            }
        }
        (val + (offset as u64)) & 0xffff_ffff_ffff
    }
}

impl Drop for RDPMC {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(
                self.buf as *mut libc::c_void,
                libc::sysconf(libc::_SC_PAGESIZE) as usize,
            );
            libc::close(self.fd);
        }
    }
}

//#[cfg(test)]
//mod tests {
//    use crate::x86::{PmuX86, RDPMC};
//
//    #[test]
//    fn rdpmc() {
//        let counter = RDPMC::open_raw_desc(0, 0, -1);
//        assert!(counter.is_ok());
//        let mut counter = counter.unwrap();
//
//        let mut prev = counter.read();
//        let thresh = 1000;
//
//        loop {
//            let next = counter.read();
//            if next - prev > thresh {
//                println!("{}, {}", next, prev);
//                break;
//            }
//            prev = next;
//            std::thread::sleep(std::time::Duration::from_secs(1));
//        }
//    }
//}
