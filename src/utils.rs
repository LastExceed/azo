pub mod com;

use std::ffi::*;
use std::marker::PhantomData;
use windows_core::Interface;
use super::*;

/// Can't use [`From`] / [`Into`] because of the orphan rule
pub(crate) fn create_result<T>(ok_value: T, code: ResultCode) -> Result<T> {
    match code {
        ResultCode::OK |
        ResultCode::SUCCESS => Ok(ok_value),
        
        bad_code => Err(Error(unsafe { NonZeroI32::new_unchecked(bad_code.0) }))
    }
}

/// Somehow [`CString`] has no equivalent of [`CStr::from_bytes_until_nul`] - <https://github.com/rust-lang/rust/pull/96186>
#[must_use]
pub(crate) fn cstring_from_bytes_until_nul(buffer: &[u8]) -> CString {
    CStr
    ::from_bytes_until_nul(buffer)
    .expect("buffer overflow")
    .to_owned()
}

/// workaround until `#![feature(negative_impls)]` gets stabilized
type PhantomUnSend = PhantomData<*const ()>;