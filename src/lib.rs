//! Utilities to interact with linux perf kernel/userspace APIs and microarchitectural PMUs.

#![deny(missing_docs, missing_debug_implementations)]

mod errors;
pub use errors::{Error, Result};

pub mod perf;
pub use perf::ffi;

pub mod pmu;

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

#[allow(dead_code)]
pub(crate) mod util;
