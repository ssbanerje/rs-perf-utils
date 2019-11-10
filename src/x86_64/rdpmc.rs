//! Utilities specific to the x86_64 architecture.

use crate::perf::*;
use crate::Result;

extern "C" {
    fn rdpmc(counter: u32) -> u64;
}

/*
unsafe fn rdpmc(counter: i32) -> i64 {
    let mut low = 0i32;
    let mut high = 0i32;
    asm!("rdpmc" : "=a" (low), "=d" (high) : "c" (counter));
    (low as u64) | ((high as u64) << 32)
}
*/

/// Allows accessing CPU performance counters from ring 3 using the `perf_events` subsystem.
#[derive(Debug)]
pub struct Rdpmc {
    /// File descriptor to performance counter.
    fd: libc::c_int,
    /// Memory mapped structure storing the performance counter.
    buf: *mut ffi::perf_event_mmap_page,
}

impl Rdpmc {
    /// Initialize the performance counter using its a `perf_event_attr`.
    ///
    /// This does not check if the input `attr` corresponds to a CPU event that can be read through
    /// the `rdpmc` instruction.
    pub fn open(
        attr: &ffi::perf_event_attr,
        leader: Option<libc::c_int>,
        pid: libc::pid_t,
        cpuid: libc::c_int,
    ) -> Result<Rdpmc> {
        let mut new_ctr = Rdpmc {
            fd: perf_event_open(
                attr,
                pid,
                cpuid,
                match leader {
                    Some(x) => x,
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
        if new_ctr.buf != libc::MAP_FAILED as _ {
            Ok(new_ctr)
        } else {
            Err((libc::MAP_FAILED as i32).into())
        }
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

impl Drop for Rdpmc {
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

#[cfg(test)]
mod tests {
    use crate::x86_64::*;

    #[test]
    fn test_rdpmc() {
        let pmu_events_path = std::env::var("PMU_EVENTS").unwrap();
        let pmu = crate::Pmu::from_local_cpu(pmu_events_path).unwrap();
        let event = pmu
            .find_pmu_by_name(r"INST_RETIRED.ANY")
            .unwrap()
            .pop()
            .unwrap();
        let mut attr = event.to_perf_event_attr().pop().unwrap();
        attr.sample_type = crate::perf::ffi::perf_event_sample_format::PERF_SAMPLE_READ as _;
        attr.set_exclude_kernel(1);
        attr.set_exclude_hv(1);

        let counter = Rdpmc::open(&attr, None, 0, -1);
        assert!(counter.is_ok());
        let mut counter = counter.unwrap();

        let mut prev = counter.read();
        let thresh = 1000;
        loop {
            let next = counter.read();
            if next - prev > thresh {
                println!("{}, {}", next, prev);
                break;
            }
            prev = next;
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}
