use std::ffi::*;
use windows_core::{GUID, Interface};
use crate::windows_bindings::*;

/// Same as [`Interface::cast`], except that the target interface's IID is decoupled from its type.
pub unsafe fn cast_decoupled<Target: Interface>(interface: &impl Interface, target_iid: *const GUID) -> windows_core::Result<Target> {
    let mut out = None;
    unsafe { interface.query(target_iid, (&raw mut out).cast()) }.ok()?;
    out.ok_or_else(|| E_POINTER.into())
}

#[must_use]
pub fn convert_cstring(buffer: &[u8]) -> String {
    CStr
    ::from_bytes_until_nul(buffer)
    .expect("buffer overflow")
    .to_string_lossy()
    .into_owned()
}

/// This ZST ensures correct pairing of calls to [`CoInitializeEx`] and [`CoUninitialize`]
#[derive(Debug)]
pub struct COM(()); // private field to prevent manual construction

impl COM {
	pub fn new(coinit: COINIT) -> windows_core::Result<Self> {
		unsafe { CoInitializeEx(None, coinit) }.ok()?;
		Ok(Self(()))
	}
}

impl Drop for COM {
	fn drop(&mut self) {
		// SAFETY:
		// if init had failed, `self` would not have been created in the first place
		unsafe { CoUninitialize(); }
	}
}