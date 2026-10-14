use std::collections::HashMap;
use std::fmt::Debug;
use std::mem::MaybeUninit;
use std::sync::LazyLock;
use std::{ptr, thread};
use tap::Pipe;
use windows_core::{GUID, w};
use crate::host::utils::MAKEINTATOM;
use crate::utils::ResultCodeExt;
use crate::driver::{self, Driver};
use crate::win::*;

use super::utils::SendHWND;
use super::{Method, Token, WM_CALL_METHOD, WM_CREATE_DRIVER, WM_RELEASE_DRIVER};

static WINDOW_CLASS: LazyLock<ATOM> = LazyLock::new(||
	WNDCLASSW {
		lpfnWndProc  : Some(window_procedure),
		hInstance    : unsafe { GetModuleHandleW(None) },
		lpszClassName: w!("azo-host"),
		..Default::default()
	}
	.pipe(|class| unsafe { RegisterClassW(&raw const class) })
);

pub fn spawn() -> oneshot::Receiver<SendHWND> {
	let (sender, receiver) = oneshot::channel::<SendHWND>();
		
	thread::spawn(move || {
		let window_handle = unsafe { CreateWindowExW(0, MAKEINTATOM(WINDOW_CLASS.0), None, 0, 0, 0, 0, 0, Some(HWND_MESSAGE), None, None, None) };
		sender.send(SendHWND(window_handle)).expect("receiver dropped prematurely");

		let mut context = Context::default();
		
		unsafe {
			SetWindowLongPtrW(
				window_handle,
				GWLP_USERDATA,
				(&raw mut context).expose_provenance().cast_signed()
			);
		}
		
		let init_result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED as _) };
		assert_eq!(init_result, S_OK, "COM STA initialization on a fresh thread should be infallible");

		pump();
		
		unsafe { CoUninitialize(); }
	});
	
	receiver
}

fn pump() {
	loop {
		let mut msg = MSG::default();
		let bool_result = unsafe { GetMessageW(&raw mut msg, None, 0, 0) }; // contrary to what the return type suggests, this is fallible
		
		match bool_result.0 {
			-1 => (), // ignore invalid messages
			0  => break, // WM_QUIT received	
			_  => unsafe { DispatchMessageW(&raw const msg); }
		}
	}
}

unsafe extern "system" fn window_procedure(window_handle: HWND, message: u32, in_: WPARAM, out: LPARAM) -> LRESULT {
	// cannot get the context before matching the message since it might not exist yet (WM_NCCREATE, WM_CREATE, ...)
	#[deny(nonstandard_style, reason = "prevent accidental wildcard parrterns")]
	match message as i32 {
		WM_CREATE_DRIVER  => Context::get(window_handle).create_driver (in_, out),
		WM_CALL_METHOD    => Context::get(window_handle).call_method   (in_, out),
		WM_RELEASE_DRIVER => Context::get(window_handle).release_driver(in_),
		WM_DESTROY        => unsafe { PostQuitMessage(0) },
		_                 => return unsafe { DefWindowProcW(window_handle, message, in_, out) }
	}

	LRESULT::default()
}

fn write_response<Out>(l_param: LPARAM, value: Out) {
	l_param
	.0
	.cast_unsigned()
	.pipe(ptr::with_exposed_provenance_mut::<MaybeUninit<Out>>)
	.pipe(|ptr| unsafe { ptr.as_mut() })
	.expect("invalid LParam")
	.write(value);
}

#[derive(Debug, Default)]
struct Context {
	next_token_val: usize,
	drivers: HashMap<Token, driver::SafeHandle>
}

impl Context {
	fn get<'ctx>(window_handle: HWND) -> &'ctx mut Self {
		unsafe {
			GetWindowLongPtrW(window_handle, GWLP_USERDATA)
			.cast_unsigned()
			.pipe(ptr::with_exposed_provenance_mut::<Self>)
			.as_mut()
			.expect("invalid GWLP_USERDATA")
		}
	}
	
	fn create_driver(&mut self, in_: WPARAM, out: LPARAM) {
		in_
		.0
		.pipe(ptr::with_exposed_provenance::<GUID>)
		.pipe(|ptr| unsafe { ptr.as_ref() })
		.expect("invalid WParam")
		.pipe(driver::SafeHandle::new)
		.map(|driver| self.register_driver(driver))
		.pipe(|result| write_response(out, result));
	}
	
	fn register_driver(&mut self, driver: driver::SafeHandle) -> Token {
		let token = self.create_token();
		self.drivers.insert(token, driver);
		token
	}
	
	const fn create_token(&mut self) -> Token {
		let token = Token(self.next_token_val);
		self.next_token_val += 1;
		token
	}
	
	fn call_method(&self, in_: WPARAM, out: LPARAM) {
		let (token, method) =
			in_
			.0
			.pipe(ptr::with_exposed_provenance_mut::<Option<(Token, Method)>>)
			.pipe(|ptr| unsafe { ptr.as_mut() })
			.expect("invalid WParam")
			.take()
			.expect("command missing");
		
		let driver = &self.drivers[&token];
				
		match method {
			Method::Init { window_handle }          => driver.init              (window_handle).pipe(|ret| write_response(out, ret)),
			Method::Name                            => driver.name              (             ).pipe(|ret| write_response(out, ret)),
			Method::Version                         => driver.version           (             ).pipe(|ret| write_response(out, ret)),
			Method::LastError                       => driver.last_error        (             ).pipe(|ret| write_response(out, ret)),
			Method::Start                           => driver.start             (             ).pipe(|ret| write_response(out, ret)),
			Method::Stop                            => driver.stop              (             ).pipe(|ret| write_response(out, ret)),
			Method::ChannelCounts                   => driver.channel_counts    (             ).pipe(|ret| write_response(out, ret)),
			Method::Latencies                       => driver.latencies         (             ).pipe(|ret| write_response(out, ret)),
			Method::BufferSize                      => driver.buffer_size       (             ).pipe(|ret| write_response(out, ret)),
			Method::CanSampleRate { sample_rate }   => driver.can_sample_rate   (sample_rate  ).pipe(|ret| write_response(out, ret)),
			Method::GetSampleRate                   => driver.get_sample_rate   (             ).pipe(|ret| write_response(out, ret)),
			Method::SetSampleRate { sample_rate }   => driver.set_sample_rate   (sample_rate  ).pipe(|ret| write_response(out, ret)),
			Method::ClockSources                    => driver.clock_sources     (             ).pipe(|ret| write_response(out, ret)),
			Method::SetClockSource { clock_source } => driver.set_clock_source  (clock_source ).pipe(|ret| write_response(out, ret)),
			Method::SamplePosition                  => driver.sample_position   (             ).pipe(|ret| write_response(out, ret)),
			Method::ChannelInfo { channel_id }      => driver.channel_info      (channel_id   ).pipe(|ret| write_response(out, ret)),
			Method::DisposeBuffers                  => driver.dispose_buffers   (             ).pipe(|ret| write_response(out, ret)),
			Method::OpenControlPanel                => driver.open_control_panel(             ).pipe(|ret| write_response(out, ret)),
			Method::OutputReady                     => driver.output_ready      (             ).pipe(|ret| write_response(out, ret)),
			Method::CreateBuffers { channels, buffer_size, callbacks } => unsafe {
				driver
				.create_buffers(channels, buffer_size, callbacks)
				.map(Iterator::collect::<Vec<_>>)
				.pipe(|ret| write_response(out, ret));
			},
			Method::Future { selector, opt } => unsafe {
				driver
				.as_unsafe()
				.0
				.future(selector, opt)
				.to_result()
				.pipe(|ret| write_response(out, ret));
			},
		}
	}
	
	fn release_driver(&mut self, in_: WPARAM) {
		drop(self.drivers.remove(&Token(in_.0)));
	}
}