use log::info;
use regex::Regex;
use std::io::{BufRead, BufReader};

/// Get identifier string for the CPU
pub fn get_cpu_string() -> crate::Result<String> {
    Ok(arch_specific_cpustr())
}

extern "C" {
    #[cfg(target_arch = "x86_64")]
    fn cpuid(op: u32, a: *mut u32, b: *mut u32, c: *mut u32, d: *mut u32);
    #[cfg(target_arch = "powerpc64")]
    fn mfspr_pvr() -> u32;
}

/// Get cpuid for x86 processors.
#[cfg(target_arch = "x86_64")]
fn arch_specific_cpustr() -> String {
    let mut lvl = 0u32;
    let mut a = 0u32;
    let mut b = 0u32;
    let mut c = 0u32;
    let mut d = 0u32;
    let mut vendor = [0u8; 12];
    let mut family = 0u32;
    let mut model = 0u32;
    let mut step = 0u32;

    unsafe {
        cpuid(
            0,
            &mut lvl as *mut u32,
            &mut b as *mut u32,
            &mut c as *mut u32,
            &mut d as *mut u32,
        );
        std::ptr::copy_nonoverlapping(&b as *const u32 as *const u8, &mut vendor[0] as *mut u8, 4);
        std::ptr::copy_nonoverlapping(&d as *const u32 as *const u8, &mut vendor[4] as *mut u8, 4);
        std::ptr::copy_nonoverlapping(&c as *const u32 as *const u8, &mut vendor[8] as *mut u8, 4);
    }

    if lvl >= 1 {
        unsafe {
            cpuid(
                1,
                &mut a as *mut u32,
                &mut b as *mut u32,
                &mut c as *mut u32,
                &mut d as *mut u32,
            );
        }
        family = (a >> 8) & 0xf;
        model = (a >> 4) & 0xf;
        step = a & 0xf;
        if family == 0xf {
            family += (a >> 20) & 0xff;
        }
        if family >= 0x6 {
            model += ((a >> 16) & 0xf) << 4;
        }
    }

    format!(
        "{}-{:X}-{:X}-{:X}",
        std::str::from_utf8(&vendor).unwrap(),
        family,
        model,
        step
    )
}

/// Get cpuid for ppc64 processors.
#[cfg(target_arch = "powerpc64")]
fn arch_specific_cpustr() -> String {
    let pvr = unsafe { mfspr_pvr() };
    let pvr_version = (pvr >> 16) & 0xFFFF;
    let pvr_revision = (pvr >> 0) & 0xFFFF;
    format!("{:X}{:X}", pvr_version, pvr_revision)
}

#[derive(Debug, Default)]
pub struct RawEvent {
    name: Option<String>,
    event: Option<String>,
    desc: Option<String>,
    topic: Option<String>,
    long_desc: Option<String>,
    pmu: Option<String>,
    unit: Option<String>,
    perpkg: Option<String>,
    metric_expr: Option<String>,
    metric_name: Option<String>,
    metric_group: Option<String>,
}

#[derive(Debug)]
struct ParsedEvent;

#[derive(Default, Debug)]
pub struct Pmu {
    cpu_str: String,
    raw_events: Vec<RawEvent>,
    parsed_events: Vec<ParsedEvent>,
}

impl Pmu {
    /// Load PMU event information for local CPU from the specified path.
    pub fn from_local_cpu(path: String) -> crate::Result<Pmu> {
        let cpu_str = get_cpu_string()?;
        Pmu::from_cpu_str(cpu_str, path)
    }

    /// Load CPU-specific PMU information from the specified path.
    pub fn from_cpu_str(cpu: String, path: String) -> crate::Result<Pmu> {
        // Check for global events
        let mut json_files: Vec<String> = std::fs::read_dir(&path)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|x| {
                std::fs::metadata(x.path()).unwrap().is_file()
                    && x.file_name().into_string().unwrap().ends_with(".json")
            })
            .map(|x| x.file_name().into_string().unwrap())
            .collect();

        // Check mapfile for paths
        let mapfile = std::fs::File::open(format!("{}/{}", &path, "mapfile.csv"))?;
        let mapped_files = BufReader::new(mapfile)
            .lines()
            .filter_map(Result::ok)
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let splits: Vec<&str> = l.split(',').collect();
                if Regex::new(splits[0]).unwrap().is_match(&cpu) {
                    Some(String::from(splits[2]))
                } else {
                    None
                }
            })
            .flat_map(|f: String| {
                let full_path = format!("{}/{}", path, f);
                if std::fs::metadata(&full_path).unwrap().is_file() {
                    vec![full_path]
                } else {
                    std::fs::read_dir(full_path)
                        .unwrap()
                        .filter_map(|x| match x {
                            Ok(f) => Some(f.file_name().into_string().unwrap()),
                            _ => None,
                        })
                        .collect()
                }
            });
        json_files.extend(mapped_files);
        info!("Parsing PMU events from {:?}", json_files);

        // Parse JSON files
        let evts: Vec<RawEvent> = json_files
            .iter()
            .map(|f| std::fs::read_to_string(f).unwrap())
            .map(|s| serde_json::from_str(s.as_str()))
            .filter_map(Result::ok)
            .flat_map(Pmu::parse_json)
            .collect();

        Ok(Pmu {
            cpu_str: cpu,
            raw_events: evts,
            parsed_events: vec![],
        })
    }

    fn parse_json(v: serde_json::Value) -> Vec<RawEvent> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|_j| {
                let evt = RawEvent::default();
                // TODO: Implement
                evt
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::pmu::{get_cpu_string, Pmu};

    #[test]
    fn test_cpu_str() {
        let cpu_str = get_cpu_string();
        assert!(cpu_str.is_ok());
    }

    #[test]
    fn test_pmu_construction() {
        let pmu_events_path = match std::env::var("PMU_EVENTS") {
            Ok(x) => x,
            Err(_) => "perfmon".into(),
        };
        let pmu = Pmu::from_local_cpu(pmu_events_path);
        assert!(pmu.is_ok());
    }
}
