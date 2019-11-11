//! Utilities to interact with linux perf kernel/userspace APIs and microarchitectural PMUs.

#![deny(missing_docs, missing_debug_implementations)]

mod errors;
pub use errors::{Error, Result};

pub mod perf;

mod pmu;
pub use pmu::{MetricExpr, Pmu, PmuEvent, RawEvent};

mod performance_counters;
pub use performance_counters::{EventRecord, Events, MmappedRingBuffer};

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "powerpc64")]
mod powerpc64;

/// Architecture specific implementation details of performance counters.
pub mod arch_specific {
    #[cfg(target_arch = "x86_64")]
    pub use crate::x86_64::*;

    #[cfg(target_arch = "powerpc64")]
    pub use crate::powerpc64::*;
}

mod pci;
pub use pci::PciHandle;
