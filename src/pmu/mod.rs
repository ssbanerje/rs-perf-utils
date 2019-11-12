//! Utilities to read and process PMU events.

use crate::perf::PerfVersion;
use regex::Regex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};

mod events;
pub use events::{PmuEvent, RawEvent};

mod metrics;
pub use metrics::{MetricExpr, Rule};

/// Provides the ability to parse and interact with CPU specific PMU counters using their JSON descriptions.
#[derive(Default, Debug)]
pub struct Pmu {
    /// String identifying CPU.
    pub cpu_str: String,
    /// List of parsed performance counter events.
    pub events: Vec<PmuEvent>,
    /// Raw JSON-based performance counter events for the `cpu_str`.
    raw_events: Vec<RawEvent>,
}

impl Pmu {
    /// Load PMU event information for local CPU from the specified path.
    pub fn from_local_cpu(path: String) -> crate::Result<Self> {
        let cpu_str = crate::arch::get_cpu_string();
        Pmu::from_cpu_str(cpu_str, path)
    }

    /// Load CPU-specific PMU information from the specified path.
    pub fn from_cpu_str(cpu: String, path: String) -> crate::Result<Self> {
        // Check for global events
        let mut json_files: Vec<String> = std::fs::read_dir(&path)
            .unwrap()
            .filter_map(std::result::Result::ok)
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
            .filter_map(std::result::Result::ok)
            .filter(|l| !l.starts_with('#') && !l.starts_with('\n'))
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
                    std::fs::read_dir(&full_path)
                        .unwrap()
                        .filter_map(|x| match x {
                            Ok(f) => Some(format!(
                                "{}/{}",
                                &full_path,
                                f.file_name().into_string().unwrap()
                            )),
                            _ => None,
                        })
                        .collect()
                }
            });
        json_files.extend(mapped_files);

        let raw_events: Vec<RawEvent> = json_files
            .iter()
            .flat_map(|f| {
                let s = std::fs::read_to_string(f).unwrap();
                let mut j: Vec<HashMap<String, String>> = serde_json::from_str(s.as_str()).unwrap();
                j.iter_mut().for_each(|x| {
                    // Add the file name as a topic
                    let fname = std::path::Path::new(&f)
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap();
                    x.entry(String::from("Topic"))
                        .or_insert_with(|| String::from(&fname[0..fname.len() - 5]));
                });
                j
            })
            .collect();

        // Construct the Pmu
        let version = PerfVersion::get_details_from_tool()?;
        Ok(Pmu {
            cpu_str: cpu,
            events: raw_events
                .iter()
                .map(|x| PmuEvent::from_raw_event(x, &version))
                .filter_map(std::result::Result::ok)
                .collect(),
            raw_events,
        })
    }

    /// Filter all `PmuEvent`s using `predicate`.
    #[inline]
    pub fn filter_events<F>(&self, predicate: F) -> Vec<&PmuEvent>
    where
        F: FnMut(&&PmuEvent) -> bool,
    {
        self.events.iter().filter(predicate).collect()
    }

    /// Search for `PmuEvent`s by name.
    ///
    /// The `name` field of the function serves as a regex.
    #[inline]
    pub fn find_pmu_by_name(&self, name: &str) -> crate::Result<Vec<&PmuEvent>> {
        let re = Regex::new(name)?;
        Ok(self.filter_events(|x| re.is_match(&x.name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pmu_construction() {
        let pmu_events_path = std::env::var("PMU_EVENTS").unwrap();
        let pmu = Pmu::from_local_cpu(pmu_events_path);
        assert!(pmu.is_ok());
    }

    #[test]
    fn test_pmu_query() {
        let pmu_events_path = std::env::var("PMU_EVENTS").unwrap();
        let pmu = crate::Pmu::from_local_cpu(pmu_events_path).unwrap();
        let evt_name = r"INST_RETIRED.ANY";
        let event = pmu.find_pmu_by_name(&evt_name);
        assert!(event.is_ok());
        let event = event.unwrap();
        assert!(event.len() >= 1);
        let event = event.iter().next().unwrap();
        assert_eq!(event.name, evt_name);
    }
}
