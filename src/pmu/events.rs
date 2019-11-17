use crate::perf::ffi::{perf_event_attr, perf_type_id};
use crate::perf::PerfVersion;
use crate::{pmu::MetricExpr, Result};
use log::warn;

/// Raw event format represented in the JSON event files.
pub type RawEvent = std::collections::HashMap<String, String>;

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
    parsed_metric_expr: Option<MetricExpr>,
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
    pub fn from_raw_event(raw_event: &RawEvent, version: &PerfVersion) -> Result<Self> {
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
            let expr = raw_event.get("MetricExpr").unwrap().clone();
            if let Some(mg) = raw_event.get("MetricGroup") {
                evt.metric_group = Some(mg.clone());
            }
            evt.parsed_metric_expr = Some(MetricExpr::parse_str(expr.as_str())?);
            evt.metric_expr = Some(expr);
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
        assert!(self.event_code.is_some());
        if is_direct {
            if cfg!(target_arch = "x86_64") {
                assert!(self.umask.is_some());
                format!(
                    "r{:X}{:X}",
                    self.umask.unwrap() & 0xFF,
                    self.event_code.unwrap() & 0xFF
                )
            } else {
                format!("r{:X}", self.event_code.unwrap())
            }
        } else {
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
                "cpu/event={:#X}{}{}{}{}{}/",
                self.event_code.unwrap(),
                umask,
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

    /// Parse a metric event to get all the underlying PmuEvents.
    ///
    /// `events` is a reference to the entire database of events that has to be searched to find the
    /// correct `PmuEvent` corresponding to the metric.
    fn _get_metric_events<'a>(&self, events: Option<&'a Vec<PmuEvent>>) -> Vec<&'a PmuEvent> {
        assert!(self.is_metric);
        let vars = match self.parsed_metric_expr {
            Some(ref expr) => expr.get_counters(),
            _ => unreachable!(), // If this is a metric, the metric_expr must be set!
        };

        if let Some(evts) = events {
            let found: Vec<&PmuEvent> = evts
                .iter()
                .filter(|evt| vars.contains(&&evt.name))
                .collect();
            if found.is_empty() {
                warn!(
                    "Could not resolve one of {:?} in event {}, {}",
                    vars,
                    self.name,
                    evts.len()
                );
            }
            found
        } else {
            // Had no events
            vec![]
        }
    }

    /// Get string for perf command line tool from this `PmuEvent`.
    ///
    /// `events` is a reference to the entire database of events that has to be searched to find the
    /// correct `PmuEvent` corresponding to the metric. If one is sure that `self` is not a metric
    /// event,
    pub fn to_perf_string(&self, pv: &PerfVersion, events: Option<&Vec<PmuEvent>>) -> String {
        if !self.is_metric {
            if self.unit.is_none() {
                self._get_core_event_string(pv.direct(), pv.has_name())
            } else {
                self._get_uncore_event_string(pv.has_name())
            }
        } else {
            self._get_metric_events(events)
                .iter()
                .map(|x| x.to_perf_string(pv, None))
                .collect::<Vec<String>>()
                .join(",")
        }
    }

    /// Get a `perf_event_attr` corresponding to this event.
    ///
    /// If this event is a derived event, then it returns multiple `perf_event_attrs` corresponding
    /// to all events that need to be collected.
    pub fn to_perf_event_attr(&self, events: Option<&Vec<PmuEvent>>) -> Vec<perf_event_attr> {
        // TODO Unsure if this works on a PowerPC machine
        if !self.is_metric {
            let mut attr = perf_event_attr::default();
            attr.type_ = perf_type_id::PERF_TYPE_RAW as _;
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
            self._get_metric_events(events)
                .iter()
                .flat_map(|x| x.to_perf_event_attr(None))
                .collect()
        }
    }
}

impl PartialEq for PmuEvent {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pmu::Pmu;
    use rayon::prelude::*;
    use std::process::{Command, Stdio};

    #[test]
    fn test_pmuevent_to_perfstring() {
        let pmu_events_path = std::env::var("PMU_EVENTS").unwrap();
        let pmu = Pmu::from_local_cpu(pmu_events_path).unwrap();
        let pv = PerfVersion::get_details_from_tool().unwrap();
        let perf_strings: Vec<String> = pmu
            .events
            .iter()
            .map(|x| x.to_perf_string(&pv, Some(&pmu.events)))
            .collect();
        let res = perf_strings[0..100] // Otherwise this takes too long
            .par_iter()
            .all(|evt| {
                let stat = Command::new("perf")
                    .args(&["stat", "-e", evt.as_str(), "--", "/bin/true"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap();
                stat.success()
            });
        assert!(res);
    }

    #[test]
    fn test_perf_event_attr_gen() {
        let pmu_events_path = std::env::var("PMU_EVENTS").unwrap();
        let pmu = Pmu::from_local_cpu(pmu_events_path).unwrap();
        for evt in pmu.events.iter() {
            let attr = evt.to_perf_event_attr(Some(&pmu.events));
            assert!(!attr.is_empty());
        }
    }
}
