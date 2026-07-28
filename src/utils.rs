use std::ffi::*;
use windows_core::{IUnknown, Interface};
use crate::windows_bindings::*;
use super::*;

/// Can't use [`From`] / [`Into`] because of the orphan rule
pub fn create_result<T>(ok_value: T, code: ErrorCode) -> Result<T> {
    match code {
        ErrorCode::OK |
        ErrorCode::SUCCESS => Ok(ok_value),
        
        bad_code => Err(Error(unsafe { NonZeroI32::new_unchecked(bad_code.0) }))
    }
}

/// Somehow [`CString`] has no equivalent of [`CStr::from_bytes_until_nul`] - <https://github.com/rust-lang/rust/pull/96186>
#[must_use]
pub fn cstring_from_bytes_until_nul(buffer: &[u8]) -> CString {
    CStr
    ::from_bytes_until_nul(buffer)
    .expect("buffer overflow")
    .to_owned()
}

/// This ZST ensures correct pairing of calls to [`CoInitializeEx`] and [`CoUninitialize`]
#[derive(Debug)]
pub struct COM(()); // private field to prevent manual construction

impl COM {
	pub fn new(coinit: COINIT) -> windows_core::Result<Self> {
		unsafe { CoInitializeEx(None, coinit) }.ok()?;
		Ok(Self(()))
	}

    #[expect(clippy::unused_self, reason = "required as a guarantee that COM is initialized")]
    pub fn create_driver_instance(&self, guid: *const GUID) -> WinResult<IIASIORedecl> {
        // windows-rs binds this function in a way where the IID is acquired from a trait-associated constant,
        // which is impossible to implement in this case (see doc comment of `IIASIORedecl`)
        let i_unknown: IUnknown =
            unsafe { CoCreateInstance(guid, None, CLSCTX_SERVER) }?;

        // The aforementioned binding limitation also applies to `.cast()`.
        // Luckily, the underlying `.query()` is public, which enables the following work-around:
        unsafe { cast_decoupled(&i_unknown, guid) }
    }
}

/// Same as [`Interface::cast`], except that the target interface's IID is decoupled from its type.
pub unsafe fn cast_decoupled<Target: Interface>(interface: &impl Interface, target_iid: *const GUID) -> windows_core::Result<Target> {
    let mut out = None;
    unsafe { interface.query(target_iid, (&raw mut out).cast()) }.ok()?;
    out.ok_or_else(|| E_POINTER.into())
}

impl Drop for COM {
	fn drop(&mut self) {
		// SAFETY:
		// if init had failed, `self` would not have been created in the first place
		unsafe { CoUninitialize(); }
	}
}