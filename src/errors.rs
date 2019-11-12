//! Utilities dealing with error handling in this crate.

use failure::Fail;

/// Errors produced by this crate.
#[derive(Debug, Fail)]
pub enum Error {
    /// Errors originating from calls to `std::io::*`.
    #[fail(display = "IO Error - {}", _0)]
    IO(#[cause] std::io::Error),
    /// Errors originating from calls to `regex::*`.
    #[fail(display = "Regex Error - {}", _0)]
    Regex(#[cause] regex::Error),
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
    #[fail(display = "System Error - RC={}", _0)]
    System(i32),
    /// Errors in parsing PMU event JSON files.
    ///
    /// This can be because of a malformed JSON file or because parsing of some JSON formats is
    /// unimplemented.
    #[fail(display = "Error while parsing PMU JSON files.")]
    ParsePmu,
    /// Errors in finding PMU under current system configuration.
    #[fail(display = "Could not find PMU")]
    PmuNotFound,
}

impl From<std::io::Error> for Error {
    #[inline]
    fn from(err: std::io::Error) -> Self {
        Error::IO(err)
    }
}

impl From<regex::Error> for Error {
    #[inline]
    fn from(err: regex::Error) -> Self {
        Error::Regex(err)
    }
}

impl From<std::num::ParseIntError> for Error {
    #[inline]
    fn from(err: std::num::ParseIntError) -> Self {
        Error::ParseInt(err)
    }
}

impl From<std::str::Utf8Error> for Error {
    #[inline]
    fn from(err: std::str::Utf8Error) -> Self {
        Error::ParseUtf8(err)
    }
}

impl From<pest::error::Error<crate::pmu::Rule>> for Error {
    #[inline]
    fn from(err: pest::error::Error<crate::pmu::Rule>) -> Self {
        Error::ParseMetricExpr(err)
    }
}

impl From<i32> for Error {
    #[inline]
    fn from(err: i32) -> Self {
        Error::System(err)
    }
}

/// Result type used in this crate.
pub type Result<T> = std::result::Result<T, Error>;
