//! Utilities specific to the x86_64 architecture.

use crate::perf::*;
use crate::{Error, Result};

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

impl HardwareReadable for PerfEvent {
    fn read_hw(&self) -> Result<u64> {
        // Check capability
        if self.ring_buffer.is_none() {
            return Err(Error::WrongReadMethod);
        }
        let buf = if let Some(ref rb) = self.ring_buffer {
            rb.header
        } else {
            unreachable!()
        };
        unsafe {
            if (*buf).__bindgen_anon_1.__bindgen_anon_1.cap_user_rdpmc() == 0 {
                return Err(Error::PerfNotCapable);
            }
        }
        // Read counter
        let mut val: u64;
        let mut offset: i64;
        let mut seq: u32;
        let mut idx: u32;
        loop {
            let pc: &ffi::perf_event_mmap_page = unsafe { &*buf };
            seq = pc.lock;
            std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
            idx = pc.index;
            offset = pc.offset;
            if idx == 0 {
                val = 0;
                break;
            }
            val = unsafe { rdpmc(idx - 1) };
            std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
            if seq != pc.lock {
                break;
            }
        }
        Ok((val + (offset as u64)) & 0xffff_ffff_ffff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdpmc_read() -> crate::Result<()> {
        // Get perf_event_attr
        let pmu_events_path = std::env::var("PMU_EVENTS")?;
        let pmu = crate::Pmu::from_local_cpu(pmu_events_path)?;
        let attr = pmu.find_pmu_by_name(r"INST_RETIRED.ANY")?.pop().unwrap()
            .to_perf_event_attr(Some(&pmu.events)).pop().unwrap();
        let evt = PerfEvent::build()
            .start_disabled()
            .use_ring_buffer()
            .open(Some(attr))?;

        // Count
        evt.enable()?;
        for i in 1..10 {
            println!("{}", i);
        }
        let count = evt.read_hw();
        assert!(count.is_ok());
        Ok(())
    }
}
