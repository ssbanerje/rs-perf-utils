use log::info;
use perf_utils::perf::PerfEvent;
use perf_utils::registry::Pmu;
use perf_utils::{Counter, SampledCounter, ScaledValue};

fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 1,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn main() -> perf_utils::Result<()> {
    env_logger::init();

    // Get path to event metadata
    let prg_args = std::env::args().skip(1).next();
    let pmu_events_path = if let Some(a) = prg_args {
        a
    } else {
        std::env::var("PMU_EVENTS")?
    };

    // Parse metadata
    let pmu = Pmu::from_local_cpu(pmu_events_path)?;

    // Setup event
    let attrs = pmu
        .find_pmu_by_name(r"INST_RETIRED.ANY")?
        .pop()
        .unwrap()
        .to_perf_event_attr(Some(&pmu.events))?;
    let mut events = PerfEvent::build()
        .set_period(100)
        .enable_sampling()
        .start_disabled()
        .requested_size(1 << 12) // two pages
        .open_group(attrs)?;

    for run in 0..2 {
        // Start counting
        for evt in events.iter() {
            evt.reset()?;
            evt.enable()?;
        }

        // Workload
        info!("Starting workload. Run {}.", run);
        fibonacci(10);

        // Stop counting
        for evt in events.iter() {
            evt.disable()?;
        }

        // Read counters
        for evt in events.iter_mut() {
            let samples: Vec<String> = evt
                .read_samples()
                .iter()
                .map(|x| format!("{:#X}", x.scaled_value()))
                .collect();
            info!("Run {} -> {}", run, samples.join(", "))
        }
    }

    Ok(())
}
