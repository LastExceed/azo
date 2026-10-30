use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::LazyLock;
use std::{ptr, thread};
use tap::Pipe;
use windows_core::{GUID, w};
use crate::host::utils::MAKEINTATOM;
use crate::utils::ResultCodeExt;
use crate::driver::{self, Driver};
use crate::{WinResult, win::*};

use super::utils::{DataExchange, SendSyncHWND};
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

#[derive(Debug)]
pub struct Worker(SendSyncHWND);

impl Worker {
	pub fn new() -> Self {
		let (sender, receiver) = oneshot::channel::<SendSyncHWND>();
		
		thread::spawn(move || {
			let window_handle = unsafe { CreateWindowExW(0, MAKEINTATOM(WINDOW_CLASS.0), None, 0, 0, 0, 0, 0, Some(HWND_MESSAGE), None, None, None) };
			sender.send(SendSyncHWND(window_handle)).expect("host should not drop the receiver");

			let mut context = Context::default();
			context.set(window_handle);

			pump();
		});
		
		receiver
			.recv()
			.expect("worker should not drop the sender")
			.pipe(Self)
	}
	
	pub fn send(&self, message: i32, w_param: usize) {
		unsafe {
			SendMessageW(
				self.0.0,
				message as _,
				WPARAM(w_param),
				LPARAM::default()
			);
		}
	}
	
	pub fn post(&self, message: i32, w_param: usize) -> WinResult<()> {
		unsafe {
			PostMessageW(
				Some(self.0.0),
				message as _,
				WPARAM(w_param),
				LPARAM::default()
			)
		}
		.ok()
	}
}

impl Drop for Worker {
	fn drop(&mut self) {
		_ = self.post(WM_CLOSE, 0);
	}
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

unsafe extern "system" fn window_procedure(window_handle: HWND, message: u32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
	// cannot get the context before matching the message since it might not exist yet (WM_NCCREATE, WM_CREATE, ...)
	#[deny(nonstandard_style, reason = "prevent accidental wildcard patterns")]
	match message as i32 {
		WM_CREATE_DRIVER  => Context::get(window_handle).create_driver (w_param),
		WM_CALL_METHOD    => Context::get(window_handle).call_method   (w_param),
		WM_RELEASE_DRIVER => Context::get(window_handle).release_driver(w_param),
		WM_DESTROY        => unsafe { PostQuitMessage(0) },
		_                 => return unsafe { DefWindowProcW(window_handle, message, w_param, l_param) }
	}

	LRESULT::default()
}

#[derive(Debug, Default)]
struct Context {
	next_token_val: usize,
	drivers: HashMap<Token, driver::SafeHandle>
}

impl Context {
	fn set(&mut self, window_handle: HWND) {
		unsafe {
			SetWindowLongPtrW(
				window_handle,
				GWLP_USERDATA,
				<*mut _>::expose_provenance(self).cast_signed()
			);
		}
	}
	
	fn get<'ctx>(window_handle: HWND) -> &'ctx mut Self {
		unsafe {
			GetWindowLongPtrW(window_handle, GWLP_USERDATA)
			.cast_unsigned()
			.pipe(ptr::with_exposed_provenance_mut::<Self>)
			.as_mut()
			.expect("invalid GWLP_USERDATA")
		}
	}
	
	fn create_driver(&mut self, w_param: WPARAM) {
		let data_exchange = unsafe {
			ptr::with_exposed_provenance_mut::<DataExchange<&GUID, WinResult<Token>>>(w_param.0)
			.as_mut_unchecked()
		};
		
		unsafe { data_exchange.take_request() }
		.pipe(driver::SafeHandle::new)
		.map(|driver| self.register_driver(driver))
		.pipe(|result| data_exchange.write_response(result));
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
	
	fn call_method(&self, w_param: WPARAM) {
		// cannot use the [`DataExchange`] union here,
		// as the `Response` type depends on the request value
		let ptr = ptr::with_exposed_provenance_mut::<(_, _)>(w_param.0);

		let (token, method) = unsafe { ptr.read() };
		let driver = &self.drivers[&token];

		match method {
			Method::Init { window_handle }          => unsafe { *ptr.cast() = driver.init              (window_handle) },
			Method::Name                            => unsafe { *ptr.cast() = driver.name              (             ) },
			Method::Version                         => unsafe { *ptr.cast() = driver.version           (             ) },
			Method::LastError                       => unsafe { *ptr.cast() = driver.last_error        (             ) },
			Method::Start                           => unsafe { *ptr.cast() = driver.start             (             ) },
			Method::Stop                            => unsafe { *ptr.cast() = driver.stop              (             ) },
			Method::ChannelCounts                   => unsafe { *ptr.cast() = driver.channel_counts    (             ) },
			Method::Latencies                       => unsafe { *ptr.cast() = driver.latencies         (             ) },
			Method::BufferSize                      => unsafe { *ptr.cast() = driver.buffer_size       (             ) },
			Method::CanSampleRate { sample_rate }   => unsafe { *ptr.cast() = driver.can_sample_rate   (sample_rate  ) },
			Method::GetSampleRate                   => unsafe { *ptr.cast() = driver.get_sample_rate   (             ) },
			Method::SetSampleRate { sample_rate }   => unsafe { *ptr.cast() = driver.set_sample_rate   (sample_rate  ) },
			Method::ClockSources                    => unsafe { *ptr.cast() = driver.clock_sources     (             ) },
			Method::SetClockSource { clock_source } => unsafe { *ptr.cast() = driver.set_clock_source  (clock_source ) },
			Method::SamplePosition                  => unsafe { *ptr.cast() = driver.sample_position   (             ) },
			Method::ChannelInfo { channel_id }      => unsafe { *ptr.cast() = driver.channel_info      (channel_id   ) },
			Method::DisposeBuffers                  => unsafe { *ptr.cast() = driver.dispose_buffers   (             ) },
			Method::OpenControlPanel                => unsafe { *ptr.cast() = driver.open_control_panel(             ) },
			Method::OutputReady                     => unsafe { *ptr.cast() = driver.output_ready      (             ) },
			Method::CreateBuffers { channels, buffer_size, callbacks } => unsafe {
				driver
				.create_buffers(channels, buffer_size, callbacks)
				.map(Iterator::collect::<Vec<_>>)
				.pipe(|ret| *ptr.cast() = ret);
			},
			Method::Future { selector, opt } => unsafe {
				driver
				.as_unsafe()
				.0
				.future(selector, opt)
				.to_result()
				.pipe(|ret| *ptr.cast() = ret);
			},
		}
	}
	
	fn release_driver(&mut self, w_param: WPARAM) {
		w_param
		.0
		.pipe(Token)
		.pipe(|token| self.drivers.remove(&token))
		.pipe(drop);
	}
}