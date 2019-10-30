//! Utilities dealing with error handling in this crate.

use failure::Fail;

/// Custom type corresponding to Errors in this crate.
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
    /// Errors originating from calls to `libc` or other system utilties.
    #[fail(display = "System Error - RC={}", _0)]
    SystemError(i32),
    /// Errors in parsing PMU event JSON files.
    #[fail(display = "Error while parsing PMU JSON files.")]
    PmuParseError,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IO(err)
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Self {
        Error::Regex(err)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(err: std::num::ParseIntError) -> Self {
        Error::ParseInt(err)
    }
}

impl From<i32> for Error {
    fn from(err: i32) -> Self {
        Error::SystemError(err)
    }
}

/// Result type used in this crate.
pub type Result<T> = std::result::Result<T, Error>;
