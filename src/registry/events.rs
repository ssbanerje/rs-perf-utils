use crate::perf::ffi::{perf_event_attr, perf_type_id};
use crate::perf::PerfVersion;
use crate::registry::MetricExpr;
use crate::{BaseEvent, Counter, Event};
use crate::{Error, Result};
use derive_more::From;
use log::{error, warn};

/// Raw event format represented in the JSON event files.
pub type RawEvent = std::collections::HashMap<String, String>;

/// Events that can be programmed into performance counter
#[derive(Debug, From, Eq, PartialEq)]
pub(crate) enum EventWrapper {
    /// Events that can be directly programmed into performance counters.
    HPC(HPCEvent),
    /// Derived events that are counted over groups of performance counter.
    Metric(MetricEvent),
}

impl EventWrapper {
    /// Create a new `Event` by parsing data in a `RawEvent`.
    pub fn from_raw_event(revt: &RawEvent) -> Result<Self> {
        if revt.contains_key("EventName") {
            Ok(HPCEvent::from_raw_event(revt)?.into())
        } else if revt.contains_key("MetricName") {
            Ok(MetricEvent::from_raw_event(revt)?.into())
        } else {
            Err(Error::ParseEvent(revt.clone()))
        }
    }
}

/// An event that can be directly polled or sampled on a hardware performance counter.
#[derive(Debug, Default, Clone)]
pub struct HPCEvent {
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
    /// Event code corresponding to the event.
    event_code: Option<u64>,
    /// Unit mask of the event.
    ///
    /// x86_64 specific: Qualifies an event to detect a special microarchitectural condition.
    umask: Option<u64>,
    /// Counter mask of the event.
    ///
    /// x86_64 specific.
    cmask: Option<u8>,
    /// Edge detect bit.
    ///
    /// x86_64 specific.
    edge: bool,
    /// Invert counter mask flag.
    ///
    /// x86_64 specific.
    inv: bool,
    /// Additional MSRs required to program the event.
    ///
    /// x86_64 specific: The first field stores the MSR address and the second stores the value to
    /// be written.
    msr: Option<(u64, u64)>,
    /// Name given to the PMU that measures this event.
    ///
    /// The first field corresponds to the Linux name, while the second field correpsonds to the
    /// name used by the processor manufacturer.
    pmu: Option<(String, String)>,
}

impl HPCEvent {
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

    /// Create a new `HPCEvent` by parsing data in a `RawEvent`.
    pub fn from_raw_event(revt: &RawEvent) -> Result<Self> {
        let mut evt = HPCEvent::default();

        evt.name = revt.get("EventName").unwrap().clone();
        evt.topic = revt.get("Topic").expect("Topic").clone();
        if let Some(d) = revt.get("BriefDescription") {
            evt.desc = d.clone();
        }
        if let Some(d) = revt.get("PublicDescription") {
            evt.long_desc = d.clone();
        }
        let mut evt_code = 0;
        if let Some(c) = revt.get("EventCode") {
            let splits: Vec<&str> = c.split(',').collect();
            evt_code |= u64::from_str_radix(&splits[0][2..], 16)?;
        }
        if let Some(c) = revt.get("ExtSel") {
            evt_code |= u64::from_str_radix(&c.as_str()[2..], 16)? << 21;
        }
        evt.event_code = Some(evt_code);
        if let Some(u) = revt.get("UMask") {
            evt.umask = Some(u64::from_str_radix(&u[2..], 16)?);
        }
        if let Some(c) = revt.get("CounterMask") {
            evt.cmask = Some(c.parse()?);
        }
        if let Some(e) = revt.get("EdgeDetect") {
            evt.edge = (e.parse::<i32>()?) != 0;
        }
        if let Some(i) = revt.get("Invert") {
            evt.inv = (i.parse::<i32>()?) != 0;
        }
        let mut msr_idx = !0;
        let mut msr_val = !0;
        if let Some(m) = revt.get("MSRIndex") {
            let split: Vec<&str> = m.split(',').collect();
            msr_idx = if split[0].len() == 1 {
                split[0].parse()?
            } else {
                u64::from_str_radix(&split[0][2..], 16)?
            };
        }
        if let Some(val) = revt.get("MSRValue") {
            msr_val = if val.len() == 1 {
                val.parse()?
            } else {
                u64::from_str_radix(&val[2..], 16)?
            };
        }
        if msr_idx != !0 {
            evt.msr = Some((msr_idx, msr_val))
        }
        if let Some(u) = revt.get("Unit") {
            if u == "NCU" {
                evt.umask = Some(0);
                evt.event_code = Some(0xFF);
            }
            let lin = if let Some(pmu) = PmuEvent::_pmu_from_json(u.as_str()) {
                pmu.into()
            } else {
                format!("uncore_{}", u)
            };
            evt.pmu = Some((lin, u.clone()));
        }

        Ok(evt)
    }
}

impl BaseEvent for HPCEvent {
    fn name(&self) -> &str {
        &self.name
    }

    fn topic(&self) -> &str {
        &self.topic
    }

    fn desc(&self) -> &str {
        &self.desc
    }
}

impl<V, C> Event<V, C> for HPCEvent
where
    C: Counter<V>,
{
    fn get_counter(&self) -> Result<C> {
        // TODO
        Err(Error::NotImplemented)
    }
}

impl PartialEq for HPCEvent {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for HPCEvent {}

/// Derived event which is counted from several `HPCEvents`.
#[derive(Debug, Default, Clone)]
pub struct MetricEvent {
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
    /// Expression used to calculate the metric.
    expr: (MetricExpr, String),
    /// Metadata for grouping metrics.
    metric_group: Option<String>,
}

impl MetricEvent {
    /// Create a new `MetricEvent` by parsing data in a `RawEvent`.
    pub fn from_raw_event(revt: &RawEvent) -> Result<Self> {
        let mut evt = MetricEvent::default();

        evt.name = revt.get("MetricName").unwrap().clone();
        evt.topic = revt.get("Topic").expect("Topic").clone();
        if let Some(d) = revt.get("BriefDescription") {
            evt.desc = d.clone();
        }
        if let Some(d) = revt.get("PublicDescription") {
            evt.long_desc = d.clone();
        }
        let expr = revt.get("MetricExpr").unwrap().clone();
        evt.expr = (MetricExpr::parse_str(expr.as_str())?, expr);
        if let Some(mg) = revt.get("MetricGroup") {
            evt.metric_group = Some(mg.clone());
        }

        Ok(evt)
    }
}

//impl crate::EventGroup<u64> for MetricEvent {
//    fn name(&self) -> &str {
//        &self.name
//    }
//
//    fn topic(&self) -> &str {
//        &self.topic
//    }
//
//    fn desc(&self) -> &str {
//        &self.desc
//    }
//
//    fn get_counters<C>(&self) -> Vec<C>
//    where
//        C: crate::Counter<u64>,
//    {
//        // TODO
//        vec![]
//    }
//
//    fn aggregate(&self, _vals: Vec<u64>) -> Result<u64> {
//        // TODO
//        Err(Error::NotImplemented)
//    }
//}

impl PartialEq for MetricEvent {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for MetricEvent {}

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

    // Fields dealing with plain events
    event_code: Option<u64>,
    umask: Option<u64>,
    cmask: Option<u8>,
    edge: bool,
    inv: bool,
    msr: Option<u64>,
    msr_val: Option<u64>,
    pmu: Option<String>,
    unit: Option<String>,
    offcore_rsp: bool,
    ldlat: bool,
    frontend: bool,

    // Fields dealing with derived events
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
                if version.offcore() && (msr == 0x1A6 || msr == 0x1A7) {
                    evt.offcore_rsp = true;
                } else if version.ldlat() && (msr == 0x3F6) {
                    evt.ldlat = true;
                } else if msr == 0x3F7 {
                    evt.frontend = true;
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
            return Err(crate::Error::ParseEvent(raw_event.clone()));
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
            let umask = if let Some(u) = self.umask {
                format!("{:X}", u & 0xFF)
            } else {
                String::default()
            };
            let event_code = if cfg!(target_arch = "x86_64") {
                self.event_code.unwrap() & 0xFF
            } else {
                self.event_code.unwrap()
            };
            format!("r{}{:X}", umask, event_code)
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
        if !self.is_metric {
            return vec![];
        }
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
    pub fn to_perf_event_attr(
        &self,
        events: Option<&Vec<PmuEvent>>,
    ) -> Result<Vec<perf_event_attr>> {
        let evts = if !self.is_metric {
            let mut attr = perf_event_attr::default();
            attr.type_ = perf_type_id::PERF_TYPE_RAW as _;
            attr.size = std::mem::size_of::<perf_event_attr>() as _;
            if let Some(e) = self.event_code {
                if cfg!(target_arch = "x86_64") {
                    attr.config |= e & 0xFF;
                } else {
                    attr.config |= e;
                }
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
            if self.offcore_rsp {
                unsafe { attr.__bindgen_anon_3.config1 |= self.msr_val.unwrap() }
            } else if self.ldlat {
                unsafe { attr.__bindgen_anon_3.config1 |= self.msr_val.unwrap() & 0xFFFF }
            }
            if let Some(ref pmu) = self.pmu {
                glob::glob(&format!("/sys/devices/{}*/type", pmu))?
                    .filter_map(std::result::Result::ok)
                    .map(|path| {
                        let mut a = attr;
                        a.type_ = std::fs::read_to_string(&path)
                            .map(|s| s.trim().parse::<u32>().unwrap())
                            .unwrap_or_else(|_| {
                                error!("Could not read {:?} for event {}", path, self.name);
                                0
                            });
                        a
                    })
                    .collect()
            } else {
                vec![attr]
            }
        } else {
            self._get_metric_events(events)
                .iter()
                .flat_map(|x| x.to_perf_event_attr(None).unwrap_or_else(|_| vec![]))
                .collect()
        };
        Ok(evts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Pmu;
    use rayon::prelude::*;
    use std::process::{Command, Stdio};

    #[test]
    fn test_pmuevent_to_perfstring() -> Result<()> {
        let pmu_events_path = std::env::var("PMU_EVENTS")?;
        let pmu = Pmu::from_local_cpu(pmu_events_path)?;
        let pv = PerfVersion::get_details_from_tool()?;
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
        Ok(())
    }

    #[test]
    fn test_perf_event_attr_gen() -> Result<()> {
        let pmu_events_path = std::env::var("PMU_EVENTS")?;
        let pmu = Pmu::from_local_cpu(pmu_events_path)?;
        for evt in pmu.events.iter() {
            let x = evt.to_perf_event_attr(Some(&pmu.events));
            assert!(x.is_ok());
        }
        Ok(())
    }
}
