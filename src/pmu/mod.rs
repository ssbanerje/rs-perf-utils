//! Utilities to read and process PMU events.

use crate::perf::PerfVersion;
use derive_more::{Index, IndexMut, IntoIterator};
use log::error;
use regex::Regex;
use std::io::{BufRead, BufReader};

mod events;
pub use events::{Event, HPCEvent, MetricEvent, PmuEvent, RawEvent};

mod metrics;
pub use metrics::{MetricExpr, Rule};

/// Provides the ability to parse and interact with CPU specific PMU counters using their JSON descriptions.
#[derive(Default, Debug, Index, IndexMut, IntoIterator)]
pub struct Pmu {
    /// String identifying CPU.
    pub cpu_str: String,
    /// List of parsed performance counter events.
    #[index]
    #[index_mut]
    #[into_iterator(owned, ref, ref_mut)]
    pub events: Vec<PmuEvent>,
    /// Raw JSON-based performance counter events for the `cpu_str`.
    raw_events: Vec<RawEvent>,
}

/// Check if `entry` is a JSON file.
fn _is_json_file(entry: &std::fs::DirEntry) -> crate::Result<(bool, String)> {
    let file_name = entry.file_name().into_string().unwrap();
    let is_js = entry.metadata()?.is_file() && file_name.ends_with(".json");
    Ok((is_js, file_name))
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
        let mut json_files: Vec<String> = std::fs::read_dir(&path)?
            .filter_map(Result::ok)
            .filter_map(|x| match _is_json_file(&x) {
                Ok((true, f)) => Some(f),
                _ => None,
            })
            .collect();

        // Check mapfile for paths
        let mapfile = std::fs::File::open(format!("{}/{}", &path, "mapfile.csv"))?;
        let mapped_files = BufReader::new(mapfile)
            .lines()
            // Remove bad lines
            .filter_map(Result::ok)
            // Remove comments and empty lines
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('\n'))
            // Get filename from file
            .filter_map(|l| {
                let splits: Vec<&str> = l.split(',').collect();
                Regex::new(splits[0]).ok().and_then(|ref x| {
                    if x.is_match(&cpu) {
                        Some(String::from(splits[2]))
                    } else {
                        None
                    }
                })
            })
            // Check if mapfile entry is a file or a directory... in case of directory read it
            .flat_map(|f: String| {
                let full_path = format!("{}/{}", path, f);
                if std::path::Path::new(&full_path).is_file() {
                    vec![full_path]
                } else {
                    std::fs::read_dir(&full_path)
                        .map(|x| x.filter_map(Result::ok).collect())
                        .unwrap_or_else(|_| vec![])
                        .iter()
                        .filter_map(|x| match _is_json_file(x) {
                            Ok((true, x)) => Some(format!("{}/{}", &full_path, x)),
                            _ => None,
                        })
                        .collect()
                }
            });
        json_files.extend(mapped_files);

        let raw_events: Vec<RawEvent> = json_files
            .iter()
            .flat_map(|f| {
                let s = std::fs::read_to_string(f).unwrap_or_else(|_| String::default());
                let mut j: Vec<RawEvent> = match serde_json::from_str(&s) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Could not parse JSON file -- {:?}", e);
                        vec![]
                    }
                };
                j.iter_mut().for_each(|x| {
                    // Add the file name as a topic
                    let fname = std::path::Path::new(&f)
                        .file_name()
                        .and_then(|x| x.to_str())
                        .unwrap_or("");
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
    pub fn filter_events<F>(&self, predicate: F) -> Vec<&PmuEvent>
    where
        F: FnMut(&&PmuEvent) -> bool,
    {
        self.events.iter().filter(predicate).collect()
    }

    /// Search for `PmuEvent`s by name.
    ///
    /// The `name` field of the function serves as a regex.
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

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_pmu_query() {
        let pmu_events_path = std::env::var("PMU_EVENTS").unwrap();
        let pmu = Pmu::from_local_cpu(pmu_events_path).unwrap();
        let evt_name = r"INST_RETIRED.ANY";
        let event = pmu.find_pmu_by_name(&evt_name);
        assert!(event.is_ok());
        let event = event.unwrap();
        assert!(event.len() >= 1);
        let event = event.iter().next().unwrap();
        assert_eq!(event.name, evt_name);
    }
}
