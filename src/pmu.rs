//! Utilities to read and process PMU events.

use crate::perf;
use crate::Result;
use regex::Regex;
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::{BufRead, BufReader};

/// Raw event format represented in the JSON event files.
pub type RawEvent = HashMap<String, String>;

/// Abstraction for a performance counter event.
#[derive(Debug, Default, Clone)]
pub struct PmuEvent {
    /// Name of the event.
    pub name: String,
    /// Topic of the event.
    ///
    /// This is the name of the JSON file from which the event was parsed.
    pub topic: String,
    /// Brief summary of the event.
    pub desc: String,
    /// Long description of the event.
    pub long_desc: String,
    /// Stores whether this `PmuEvent` is derived from several other events.
    pub is_metric: bool,

    event_code: Option<u64>,
    umask: Option<u64>,
    cmask: Option<u8>,
    edge: bool,
    inv: bool,
    per_pkg: Option<String>,
    pebs: Option<i32>,
    msr: Option<u64>,
    msr_val: Option<u64>,
    filter: Option<String>,
    pmu: Option<String>,
    unit: Option<String>,
    extra: Option<String>,

    metric_group: Option<String>,
    metric_expr: Option<String>,
}

impl PmuEvent {
    /// Get Linux PMU names from `Unit` names in the JSON.
    fn _pmu_from_json(val: &str) -> Option<&'static str> {
        match val {
            "CBO" => Some("uncore_cbox"),
            "NCU" => Some("uncore_cbox_0"),
            "QPI LL" => Some("uncore_qpi"),
            "SBO" => Some("uncore_sbox"),
            "iMPH-U" => Some("uncore_arb"),
            "CPU-M-CF" => Some("cpum_cf"),
            "CPU-M-SF" => Some("cpum_sf"),
            "UPI LL" => Some("uncore_upi"),
            "hisi_sccl,ddrc" => Some("hisi_sccl,ddrc"),
            "hisi_sccl,hha" => Some("hisi_sccl,hha"),
            "hisi_sccl,l3c" => Some("hisi_sccl,l3c"),
            "L3PMC" => Some("amd_l3"),
            _ => None,
        }
    }

    /// Create a new `PmuEvent` from a `RawEvent`.
    pub fn from_raw_event(raw_event: &RawEvent, version: &perf::PerfVersion) -> Result<Self> {
        let mut evt = PmuEvent::default();

        if let Some(n) = raw_event.get("EventName") {
            // This is a plain event
            evt.is_metric = false;
            evt.name = n.clone();
            let mut evt_code = 0;
            if let Some(c) = raw_event.get("EventCode") {
                let splits: Vec<&str> = c.split(',').collect();
                evt_code |= u64::from_str_radix(&splits[0][2..], 16)?;
            }
            if let Some(c) = raw_event.get("ExtSel") {
                evt_code |= u64::from_str_radix(&c.as_str()[2..], 16)? << 21;
            }
            evt.event_code = Some(evt_code);
            if let Some(u) = raw_event.get("UMask") {
                evt.umask = Some(u64::from_str_radix(&u[2..], 16)?);
            }
            if let Some(c) = raw_event.get("CounterMask") {
                evt.cmask = Some(c.parse()?);
            }
            if let Some(e) = raw_event.get("EdgeDetect") {
                evt.edge = (e.parse::<i32>()?) != 0;
            }
            if let Some(i) = raw_event.get("Invert") {
                evt.inv = (i.parse::<i32>()?) != 0;
            }
            if let Some(s) = raw_event.get("ScaleUnit") {
                evt.unit = Some(s.clone());
            }
            if let Some(p) = raw_event.get("PerPkg") {
                evt.per_pkg = Some(p.clone());
            }
            if let Some(p) = raw_event.get("PEBS") {
                evt.pebs = Some(p.parse()?);
            }
            if let Some(f) = raw_event.get("Filter") {
                evt.filter = Some(f.clone());
            }
            if let Some(msr) = raw_event.get("MSRIndex") {
                let split: Vec<&str> = msr.split(',').collect();
                evt.msr = if split[0].len() == 1 {
                    Some(split[0].parse()?)
                } else {
                    Some(u64::from_str_radix(&split[0][2..], 16)?)
                };
            }
            if let Some(val) = raw_event.get("MSRValue") {
                evt.msr_val = if val.len() == 1 {
                    Some(val.parse()?)
                } else {
                    Some(u64::from_str_radix(&val[2..], 16)?)
                };

                let msr = evt.msr.unwrap();
                let msr_val = evt.msr_val.unwrap();
                if version.offcore() && (msr == 0x1A6 || msr == 0x1A7) {
                    evt.extra = Some(format!(",offcore_rsp={:#X}", msr_val));
                } else if version.ldlat() && (msr == 0x3F6) {
                    evt.extra = Some(format!(",ldlat={:#X}", msr_val));
                } else if msr == 0x3F7 {
                    evt.extra = Some(format!(",frontend={:#X}", msr_val));
                }
            }
            if let Some(u) = raw_event.get("Unit") {
                if u == "NCU" {
                    evt.umask = Some(0);
                    evt.event_code = Some(0xFF);
                }
                evt.unit = Some(u.clone());
                evt.pmu = if let Some(pmu) = PmuEvent::_pmu_from_json(u.as_str()) {
                    Some(String::from(pmu))
                } else {
                    Some(format!("uncore_{}", u))
                };
            }
        } else if let Some(n) = raw_event.get("MetricName") {
            // This is derived event
            evt.is_metric = true;
            evt.name = n.clone();
            evt.metric_group = Some(raw_event.get("MetricGroup").unwrap().clone());
            evt.metric_expr = Some(raw_event.get("MetricExpr").unwrap().clone());
        } else {
            return Err(crate::Error::ParsePmu);
        }
        evt.topic = raw_event.get("Topic").expect("Topic").clone();
        if let Some(d) = raw_event.get("BriefDescription") {
            evt.desc = d.clone();
        }
        if let Some(d) = raw_event.get("PublicDescription") {
            evt.long_desc = d.clone();
        }

        // All done
        Ok(evt)
    }

    /// Perf strings for core events.
    fn _get_core_event_string(&self, is_direct: bool, put_name: bool) -> String {
        if is_direct {
            if cfg!(target_arch = "x86_64") {
                format!(
                    "r{:X}{:X}",
                    self.umask.unwrap() & 0xFF,
                    self.event_code.unwrap() & 0xFF
                )
            } else {
                format!("r{:X}", self.event_code.unwrap())
            }
        } else {
            let cmask = if let Some(c) = self.cmask {
                format!(",cmask={:#X}", c)
            } else {
                String::default()
            };
            let edge = if self.edge {
                String::from(",edge=1")
            } else {
                String::default()
            };
            let inv = if self.inv {
                String::from(",inv=1")
            } else {
                String::default()
            };
            let name = if put_name {
                format!(
                    ",name={}",
                    self.name
                        .replace('.', "_")
                        .replace(':', "_")
                        .replace('=', "_")
                )
            } else {
                String::default()
            };
            format!(
                "cpu/event={:#X},umask={:#X}{}{}{}{}/",
                self.event_code.unwrap(),
                self.umask.unwrap(),
                cmask,
                edge,
                inv,
                name
            )
        }
    }

    /// Perf strings for uncore events.
    fn _get_uncore_event_string(&self, put_name: bool) -> String {
        let umask = if let Some(u) = self.umask {
            format!(",umask={:#X}", u)
        } else {
            String::default()
        };
        let cmask = if let Some(c) = self.cmask {
            format!(",cmask={:#X}", c)
        } else {
            String::default()
        };
        let edge = if self.edge {
            String::from(",edge=1")
        } else {
            String::default()
        };
        let inv = if self.inv {
            String::from(",inv=1")
        } else {
            String::default()
        };
        let name = if put_name {
            format!(",name={}_NUM", self.name.replace(".", "_"))
        } else {
            String::default()
        };
        format!(
            "{}/event={:#X}{}{}{}{}{}/",
            match self.pmu {
                Some(ref p) => p,
                _ => unreachable!(),
            },
            self.event_code.unwrap(),
            umask,
            cmask,
            edge,
            inv,
            name
        )
    }

    /// Get string for perf command line tool from this `PmuEvent`.
    pub fn to_perf_string(&self, pv: &perf::PerfVersion) -> String {
        if !self.is_metric {
            if self.unit.is_none() {
                self._get_core_event_string(pv.direct(), pv.has_name())
            } else {
                self._get_uncore_event_string(pv.has_name())
            }
        } else {
            // TODO Implement metrics
            unimplemented!()
        }
    }

    /// Get a `perf_event_attr` corresponding to this event.
    ///
    /// If this event is a derived event, then it returns multiple `perf_event_attrs` corresponding
    /// to all events that need to be collected.
    pub fn to_perf_event_attr(&self) -> Vec<perf::ffi::perf_event_attr> {
        // TODO Unsure if this works on a PowerPC machine
        if !self.is_metric {
            let mut attr = perf::ffi::perf_event_attr::default();
            attr.size = std::mem::size_of_val(&attr).try_into().unwrap();
            attr.type_ = perf::ffi::perf_type_id::PERF_TYPE_RAW as _;
            if let Some(e) = self.event_code {
                attr.config |= e & 0xFF;
            }
            if let Some(u) = self.umask {
                attr.config |= (u as u64 & 0xFF) << 8;
            }
            if let Some(c) = self.cmask {
                attr.config |= (c as u64 & 0xF) << 24;
            }
            if self.inv {
                attr.config |= 1u64 << 23;
            }
            if self.edge {
                attr.config |= 1u64 << 18;
            }
            if let Some(ref extra) = self.extra {
                if extra.contains("offcore_rsp") {
                    unsafe { attr.__bindgen_anon_3.config1 |= self.msr_val.unwrap() }
                } else if extra.contains("ldlat") {
                    unsafe { attr.__bindgen_anon_3.config1 |= self.msr_val.unwrap() & 0xFFFF }
                }
            }
            if let Some(ref pmu) = self.pmu {
                glob::glob(&format!("/sys/devices/{}*/type", pmu))
                    .unwrap()
                    .filter_map(std::result::Result::ok)
                    .map(|x| {
                        let mut a = attr;
                        a.type_ = std::fs::read_to_string(&x)
                            .unwrap()
                            .trim()
                            .parse::<u32>()
                            .unwrap();
                        a
                    })
                    .collect()
            } else {
                vec![attr]
            }
        } else {
            // TODO Implement
            unimplemented!()
        }
    }
}

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
    pub fn from_local_cpu(path: String) -> Result<Self> {
        let cpu_str = crate::arch_specific::get_cpu_string();
        Pmu::from_cpu_str(cpu_str, path)
    }

    /// Load CPU-specific PMU information from the specified path.
    pub fn from_cpu_str(cpu: String, path: String) -> Result<Self> {
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
        let version = perf::PerfVersion::get_details_from_tool()?;
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
    pub fn find_pmu_by_name(&self, name: &str) -> Result<Vec<&PmuEvent>> {
        let re = Regex::new(name)?;
        Ok(self.filter_events(|x| re.is_match(&x.name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perf;
    use std::process::{Command, Stdio};

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

    #[test]
    fn test_pmuevent_to_perfstring() {
        let pmu_events_path = std::env::var("PMU_EVENTS").unwrap();
        let pmu = Pmu::from_local_cpu(pmu_events_path).unwrap();
        let pv = perf::PerfVersion::get_details_from_tool().unwrap();
        let perf_strings: Vec<String> = pmu
            .events
            .iter()
            .map(|x| x.to_perf_string(&pv))
            .filter(|x| !x.is_empty())
            .collect();
        for evt in perf_strings.iter() {
            let stat = Command::new("perf")
                .args(&["stat", "-e", evt.as_str(), "--", "ls"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(stat.success())
        }
    }

    #[test]
    fn test_perf_event_attr_gen() {
        let pmu_events_path = std::env::var("PMU_EVENTS").unwrap();
        let pmu = Pmu::from_local_cpu(pmu_events_path).unwrap();
        for evt in pmu.events.iter() {
            let attr = evt.to_perf_event_attr();
            assert!(!attr.is_empty());
        }
    }
}
