//! Utilities dealing with error handling in this crate.

use derive_more::From;
use failure::Fail;

/// Errors produced by this crate.
#[derive(Debug, Fail, From)]
pub enum Error {
    /// Errors originating from calls to `std::io::*`.
    #[fail(display = "IO Error - {}", _0)]
    IO(#[cause] std::io::Error),
    /// Errors originating from calls to `std::env::*`.
    #[fail(display = "Env Error - {}", _0)]
    Env(#[cause] std::env::VarError),
    /// Errors originating from calls to `regex::*`.
    #[fail(display = "Regex Error - {}", _0)]
    Regex(#[cause] regex::Error),
    /// Errors parsing Glob patterns.
    #[fail(display = "Glob Error - {}", _0)]
    GlobPattern(#[cause] glob::PatternError),
    /// Errors interating over entries in a glob.
    #[fail(display = "Glob Error - {}", _0)]
    GlobIter(#[cause] glob::GlobError),
    /// Errors caused by parsing integers from strings.
    #[fail(display = "Parse Error - {}", _0)]
    ParseInt(#[cause] std::num::ParseIntError),
    /// Errors caused by failing to read a `&[u8]` to a `str`.
    #[fail(display = "Parse Error - {}", _0)]
    ParseUtf8(#[cause] std::str::Utf8Error),
    /// Errors caused by malformed metric expression strings for PMU events.
    #[fail(display = "Parse Error - {}", _0)]
    ParseMetricExpr(#[cause] pest::error::Error<crate::registry::Rule>),
    /// Errors originating from calls to `libc` or other system utilties.
    #[fail(display = "System Error - {}", _0)]
    System(#[cause] nix::Error),
    /// Errors in parsing PMU event JSON files.
    ///
    /// This can be because of a malformed JSON file or because parsing of some JSON formats is
    /// unimplemented.
    #[fail(display = "Error while parsing PMU JSON files - {:?}", _0)]
    ParseEvent(crate::registry::RawEvent),
    /// Caused when a `None` value is read.
    #[fail(display = "Tried to read a None value")]
    NoneError,
    /// Errors caused by capability checks on the kernel.
    #[fail(display = "Not allowed by kernel")]
    KernelCapabilityError,
    /// Errors caused executing features that are not not implemented yet.
    #[fail(display = "Not implemented")]
    NotImplemented,
}

impl Error {
    /// Create a new instance of error from the `errno` variable.
    #[inline]
    pub fn from_errno() -> Self {
        Error::System(nix::Error::Sys(nix::errno::Errno::last()))
    }
}

/// Result type used in this crate.
pub type Result<T> = std::result::Result<T, Error>;
