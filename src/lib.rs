//! Utilities to interact with linux perf kernel/userspace APIs and microarchitectural PMUs.

#![deny(missing_docs, missing_debug_implementations)]

mod errors;
pub use errors::{Error, Result};

mod pmu;
pub use pmu::{Pmu, ParsedEvent};

pub mod perf;

mod performance_counters;
pub use performance_counters::{MmappedRingBuffer, Events, EventRecord};

mod x86;
pub use x86::RDPMC;
