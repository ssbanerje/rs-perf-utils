//! Interfaces that deal with the kernel and userspace perf utilities.

pub mod ffi;

mod version;
pub use version::PerfVersion;

mod event;
pub use event::{HardwareReadable, OsReadable, PerfEvent, PerfEventBuilder, PerfEventValue};

mod mmap;
pub(crate) use mmap::PAGE_SIZE;
pub use mmap::{
    CommRecord, ContextSwitchRecord, LostRecord, Mmap2Record, ParsedRecord, ProcessRecord,
    RawRecord, RingBuffer, RingBufferIter, SampleRecord, ThrottleRecord,
};

/// Allow conversion of an event to a Linux perf event string.
pub trait ToPerfString<V, C>
where
    Self: crate::Event<V, C>,
    C: crate::Counter<V>,
{
    /// Perform conversion.
    fn to_perf_string(&self) -> String;
}
