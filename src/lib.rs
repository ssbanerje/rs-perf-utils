///! Utilities to interact with linux perf kernel/userspace APIs and microarchitectural PMUs.
use error_chain::error_chain;
error_chain! {
    foreign_links {
        IO(std::io::Error);
        Regex(regex::Error);
        ParseInt(std::num::ParseIntError);
    }

     errors {
        SystemError
        PmuParseError
    }
}

/// Interfaces that deal with the kernel and userspace perf utilities.
pub mod perf;

/// Utilities to read and process PMU events.
mod pmu;
pub use pmu::Pmu;

/// Utilities to read sampled events from memory mapped ring buffer.
mod map;
pub use map::{EventRecord, Events, MmappedRingBuffer};

#[cfg(target_arch = "x86_64")]
mod x86;
#[cfg(target_arch = "x86_64")]
pub use x86::RDPMC;
