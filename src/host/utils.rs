use windows_core::PCWSTR;
use crate::win::HWND;

/// <https://github.com/microsoft/windows-rs/issues/3589>
#[expect(non_snake_case, reason = "match Windows SDK")]
pub const fn MAKEINTATOM(value: u16) -> PCWSTR {
    PCWSTR::from_raw(value as _)
}

/// A [`HWND`] that is [`Send`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SendHWND(pub HWND);

// SAFETY:
// This should be in windows-rs tbh
unsafe impl Send for SendHWND {}