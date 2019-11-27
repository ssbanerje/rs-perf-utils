//! Utilities specific to the x86_64 architecture.

use crate::perf::*;
use crate::{Error, Result};

extern "C" {
    fn rdtsc() -> u64;
    fn rdpmc(counter: u32) -> i64;
}

/// Read a counter using the `rdpmc` instruction from it's `perf_event_mmap_page`.
///
/// The function returns (value, time_enabled, time_running).
pub fn read_counter_rdpmc(buf: &ffi::perf_event_mmap_page) -> Result<(u64, u64, u64)> {
    if unsafe { buf.__bindgen_anon_1.__bindgen_anon_1.cap_user_rdpmc() == 0 } {
        return Err(Error::PerfNotCapable);
    }
    let mut res: u64;
    let mut enabled = std::num::Wrapping(0u64);
    let mut running = std::num::Wrapping(0u64);
    loop {
        // Kernel increments buf.lock so read it and issue a memory barrier to get most upto date copy.
        let seq = volatile!(buf.lock);
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);

        // In case of event multiplexing the enabled and running time need to be advanced since the
        // last write from the kernel.
        enabled.0 = volatile!(buf.time_enabled);
        running.0 = volatile!(buf.time_running);
        let delta: u64 = if unsafe { buf.__bindgen_anon_1.__bindgen_anon_1.cap_user_time() } == 1
            && enabled != running
        {
            let cycles = unsafe { rdtsc() };
            let time_shift = volatile!(buf.time_shift);
            let time_offset = volatile!(buf.time_offset);
            let time_mult = volatile!(buf.time_mult) as u64;
            let quot = cycles >> time_shift;
            let rem = cycles & ((1u64 << time_shift) - 1);
            time_offset + (quot * time_mult) + ((rem * time_mult) >> time_shift)
        } else {
            0
        };
        enabled += std::num::Wrapping(delta);

        // Check of index of register to be read. 0 means counter is not active.
        let idx = volatile!(buf.index);
        if idx == 0 {
            res = !0;
            break;
        }

        // Do the measurement + sign extend result
        let mut val = unsafe { rdpmc(idx - 1) };
        let width = volatile!(buf.pmc_width);
        val <<= 64 - width;
        val >>= 64 - width;
        // count is the counter value read by the kernel in the previously + sign extend result
        let mut count = volatile!(buf.offset);
        count <<= 64 - width;
        count >>= 64 - width;
        res = (count + val) as _;
        running += std::num::Wrapping(delta);
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);

        // Check if an update happened while this loop was executing and rety
        if seq != volatile!(buf.lock) {
            break;
        }
    }
    Ok((res, enabled.0, running.0))
}

impl HardwareReadable for PerfEvent {
    fn read_hw(&self) -> Result<PerfEventValue> {
        if let Some(ref rb) = self.ring_buffer {
            let val = read_counter_rdpmc(unsafe { &*rb.header })?;
            Ok(crate::perf::PerfEventValue {
                value: val.0,
                time_enabled: val.1,
                time_running: val.2,
                id: !0,
            })
        } else {
            Err(Error::NoneError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pmu::Pmu;

    #[test]
    fn test_rdpmc_read() -> crate::Result<()> {
        // Get perf_event_attr
        let pmu_events_path = std::env::var("PMU_EVENTS")?;
        let pmu = Pmu::from_local_cpu(pmu_events_path)?;
        let attr = pmu
            .find_pmu_by_name(r"INST_RETIRED.ANY")?
            .pop()
            .unwrap()
            .to_perf_event_attr(Some(&pmu.events))?
            .pop()
            .unwrap();
        let evt = PerfEvent::build()
            .start_disabled()
            .enable_sampling()
            .open(Some(attr))?;

        // Count
        evt.reset()?;
        evt.enable()?;
        for i in 1..10 {
            println!("{}", i);
        }
        let count = evt.read_hw();
        assert!(count.is_ok());
        Ok(())
    }
}
