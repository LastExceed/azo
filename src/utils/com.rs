use std::ops::Deref;

use crate::windows_bindings::*;
use super::*;

/// This type ensures correct pairing of calls to [`CoInitializeEx`] and [`CoUninitialize`]
#[derive(Debug)]
pub struct InitGuard<T>(T, Dropper);

impl InitGuard<()> {
	pub fn new(coinit: COINIT) -> windows_core::Result<Self> {
        unsafe { CoInitializeEx(None, coinit) }.ok()?;
		
		Ok(Self((), Dropper::default()))
	}
}

impl<T> InitGuard<T> {
	pub fn map<Mapped>(self, func: impl FnOnce(T) -> Mapped) -> InitGuard<Mapped> {
		InitGuard(func(self.0), self.1)
	}
	
	/// # Safety
	/// `T` must not outlive `Dropper`
	pub unsafe fn into_inner(self) -> (T, Dropper) {
		(self.0, self.1)
	}
}

impl<T> Deref for InitGuard<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

/// Decrements the COM init counter when dropped
#[derive(Debug, Default)]
pub struct Dropper(PhantomUnSend);

impl Drop for Dropper {
	fn drop(&mut self) {
		unsafe { CoUninitialize(); }
	}
}

/// Same as [`Interface::cast`], except that the target interface's IID is decoupled from its type.
pub(crate) unsafe fn cast_decoupled<Target: Interface>(interface: &impl Interface, target_iid: *const GUID) -> windows_core::Result<Target> {
    let mut out = None;
    unsafe { interface.query(target_iid, (&raw mut out).cast()) }.ok()?;
    out.ok_or_else(|| E_POINTER.into())
}