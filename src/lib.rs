pub mod driver;
pub mod dto;
pub mod future;
pub mod utils;
pub mod win;
#[cfg(feature = "host")]
mod host;

use std::num::NonZeroI32;
use std::fmt;
use sys::ResultCode;

pub use windows_core;
pub use azo_sys as sys;
pub use win::HWND;

/// Represents an error returned by an ASIO driver.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Error(NonZeroI32);

impl Error {
    /// guaranteed to never be [`ResultCode::OK`] / [`ResultCode::SUCCESS`]
    #[must_use]
    pub const fn code(&self) -> ResultCode {
        ResultCode(self.0.get())
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.code().fmt(f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.code().fmt(f)
    }
}

#[expect(clippy::absolute_paths, reason = "name collision")]
impl std::error::Error for Error {}

#[expect(clippy::absolute_paths, reason = "name collision")]
pub type Result<T> = std::result::Result<T, Error>;

pub type WinResult<T> = windows_core::Result<T>;