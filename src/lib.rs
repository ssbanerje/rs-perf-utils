//! Utilities to interact with linux perf kernel/userspace APIs and microarchitectural PMUs.

#![deny(missing_docs, missing_debug_implementations)]

#[allow(dead_code)]
#[macro_use]
pub(crate) mod util;

mod errors;
pub use errors::{Error, Result};

mod api;
pub use api::{
    BaseEvent, Counter, Event, EventGroup, EventRegistry, HardwareCounter, SampledCounter,
    ScaledValue,
};

#[cfg(target_os = "linux")]
pub mod perf;

pub mod registry;

/// Architecture specific implementation details of performance counters:
#[cfg(target_arch = "x86_64")]
#[path = "arch/x86_64/mod.rs"]
pub mod arch;

/// Architecture specific implementation details of performance counters:
#[cfg(target_arch = "powerpc64")]
#[path = "arch/powerpc64/mod.rs"]
pub mod arch;

mod pci;
pub use pci::PciHandle;
