use std::mem::ManuallyDrop;

use windows_core::PCWSTR;
use crate::win::HWND;

/// <https://github.com/microsoft/windows-rs/issues/3589>
#[expect(non_snake_case, reason = "match Windows SDK")]
pub const fn MAKEINTATOM(value: u16) -> PCWSTR {
    PCWSTR::from_raw(value as _)
}

/// A [`HWND`] that is [`Send`] and [`Sync`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SendSyncHWND(pub HWND);

unsafe impl Send for SendSyncHWND {}
unsafe impl Sync for SendSyncHWND {}

#[repr(C)]
pub union DataExchange<Request, Response> {
	request : ManuallyDrop<Request>,
	response: ManuallyDrop<Response>
}

impl<Request, Response> DataExchange<Request, Response> {
	pub const fn new(request: Request) -> Self {
		Self {
			request: ManuallyDrop::new(request)
		}
	}
	
	pub unsafe fn take_request(&mut self) -> Request {
		unsafe { ManuallyDrop::take(&mut self.request) }
	}
	
	pub const fn write_response(&mut self, value: Response) {
		self.response = ManuallyDrop::new(value);
	}
	
	pub unsafe fn take_response(&mut self) -> Response {
		unsafe { ManuallyDrop::take(&mut self.response) }
	}
}

/// Identifies a hosted driver instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token(pub usize);

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct TokenGenerator(usize);

impl TokenGenerator {
	pub const fn new_token(&mut self) -> Token {
		let token = Token(self.0);
		self.0 += 1;
		token
	}
}