//! Interfaces that deal with the kernel and userspace perf utilities.

pub mod ffi;

mod version;
pub use version::PerfVersion;

mod event;
pub use event::{perf_event_open, PerfEventBuilder, PerfEvent, DirectReadable, HardwareReadable};

mod mmap;
pub use mmap::{RawEvent, RingBuffer, RingBufferEvents};
