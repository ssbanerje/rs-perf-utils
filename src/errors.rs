//! Utilities dealing with error handling in this crate.

use failure::Fail;

/// Errors produced by this crate.
#[derive(Debug, Fail)]
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
    ParseMetricExpr(#[cause] pest::error::Error<crate::pmu::Rule>),
    /// Errors originating from calls to `libc` or other system utilties.
    #[fail(display = "System Error - {}", _0)]
    System(#[cause] nix::Error),
    /// Caused when a `None` value is read.
    #[fail(display = "Tried to read a None value")]
    NoneError,
    /// Errors caused by capability checks on the kernel.
    #[fail(display = "Not allowed by kernel")]
    PerfNotCapable,
    /// Errors in parsing PMU event JSON files.
    ///
    /// This can be because of a malformed JSON file or because parsing of some JSON formats is
    /// unimplemented.
    #[fail(display = "Error while parsing PMU JSON files.")]
    ParsePmu,
}

impl Error {
    /// Create a new instance of error from the `errno` variable.
    #[inline]
    pub fn from_errno() -> Self {
        Error::System(nix::Error::Sys(nix::errno::Errno::last()))
    }
}

macro_rules! error_from {
    ($et: ty => $cet: expr) => {
        impl From<$et> for Error {
            #[inline]
            fn from(err: $et) -> Self {
                $cet(err)
            }
        }
    };
}

error_from!(std::io::Error => Error::IO);
error_from!(std::env::VarError => Error::Env);
error_from!(regex::Error => Error::Regex);
error_from!(glob::PatternError => Error::GlobPattern);
error_from!(glob::GlobError => Error::GlobIter);
error_from!(std::num::ParseIntError => Error::ParseInt);
error_from!(std::str::Utf8Error => Error::ParseUtf8);
error_from!(pest::error::Error<crate::pmu::Rule> => Error::ParseMetricExpr);
error_from!(nix::Error => Error::System);

/// Result type used in this crate.
pub type Result<T> = std::result::Result<T, Error>;
