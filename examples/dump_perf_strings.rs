use log::info;
use perf_utils::perf::PerfVersion;
use perf_utils::registry::Pmu;

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
    let pv = PerfVersion::get_details_from_tool()?;

    // Get perf strings
    let perf_strings: Vec<String> = pmu
        .events
        .iter()
        .map(|x| format!("{} -> {}", x.name, x.to_perf_string(&pv, Some(&pmu.events))))
        .collect();

    // Dump perf strings
    info!("{:#?}", perf_strings);

    Ok(())
}
