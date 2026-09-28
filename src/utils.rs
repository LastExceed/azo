use std::ffi::{CStr, CString};
use std::num::NonZeroI32;
use azo_sys::ResultCode;
use crate::Error;
use windows_core::{GUID, Interface};
use crate::win::*;
#[cfg(feature = "host")]
pub use crate::host::Host;


/// Same as [`Interface::cast`], except that the target interface's IID is decoupled from its type.
pub(crate) unsafe fn cast_decoupled<Target: Interface>(interface: &impl Interface, target_iid: *const GUID) -> windows_core::Result<Target> {
    let mut out = None;
    unsafe { interface.query(target_iid, (&raw mut out).cast()) }.ok()?;
    out.ok_or_else(|| E_POINTER.into())
}

/// Somehow [`CString`] has no equivalent of [`CStr::from_bytes_until_nul`] - <https://github.com/rust-lang/rust/pull/96186>
#[must_use]
pub(crate) fn cstring_from_bytes_until_nul(buffer: &[u8]) -> CString {
    CStr
    ::from_bytes_until_nul(buffer)
    .expect("buffer overflow")
    .to_owned()
}

pub(crate) trait ResultCodeExt {
    /// Cannot use [`From`] / [`Into`] because of the orphan rule
    fn to_result(self) -> crate::Result<()>;
    fn to_result_with<T>(self, ok_fn: impl FnOnce() -> T) -> crate::Result<T>;
}

impl ResultCodeExt for ResultCode {
    fn to_result(self) -> crate::Result<()> {
        match self {
            Self::OK |
            Self::SUCCESS => Ok(()),
            
            bad_code => Err(Error(unsafe { NonZeroI32::new_unchecked(bad_code.0) }))
        }
    }
    
    fn to_result_with<T>(self, ok_fn: impl FnOnce() -> T) -> crate::Result<T> {
        self.to_result().map(|()| ok_fn())
    }
}