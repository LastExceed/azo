use std::collections::HashMap;
use std::ffi::{CString, c_long, c_void};
use std::fmt::Debug;
use std::mem::MaybeUninit;
use std::sync::{Arc, LazyLock};
use std::{ptr, thread};
use tap::Pipe;
use windows_core::{GUID, w};
use crate::utils::{MAKEINTATOM, ResultCodeExt, SendHWND};
use crate::{WinResult, driver, dto, sys};
use crate::driver::Driver;
use crate::future::AsioFuture;
use crate::win::*;

static WINDOW_CLASS: LazyLock<ATOM> = LazyLock::new(||
	WNDCLASSW {
		lpfnWndProc  : Some(window_procedure),
		hInstance    : unsafe { GetModuleHandleW(None) },
		lpszClassName: w!("azo-host"),
		..Default::default()
	}
	.pipe(|class| unsafe { RegisterClassW(&raw const class) })
);

/// A COM STA to create driver instances in, hosted by a dedicated thread driving a message pump.
/// Provides a mechanism for funneling driver interactions through the apartment owner
/// via [`Proxy`] structs, similar to COMs marshaling (which ASIO unfortunately does not support).
/// 
/// > ...what?
/// 
/// Too much technical jargon? Then [`Host`] is exactly what you need: An abstraction over a bunch of
/// arcane machinery required for correctness and thread safety, which consumers of [`azo`](crate) should not have to worry about.
#[derive(Debug)]
pub struct Host(SendHWND);

impl Host {
	pub fn new() -> Arc<Self> {
		spawn_worker()
		.recv()
		.expect("worker shouldn't drop the sender")
		.pipe(Self)
		.pipe(Arc::new)
	}
	
	/// Creates an instance of a driver in this host's STA and returns a [`Proxy`] for interacting with that driver,
	/// which can be freely shared between threads without restrictions.
	pub fn create_driver(self: &Arc<Self>, guid: &GUID) -> WinResult<Proxy> {
		let guid_raw: *const _ = guid; // coerce to raw ptr
		let mut result = MaybeUninit::<WinResult<Token>>::uninit();
		
		self.send(
			WM_CREATE_DRIVER,
			guid_raw.expose_provenance(),
			(&raw mut result).expose_provenance().cast_signed()
		);
		
		unsafe { result.assume_init() }
		.map(|token| Proxy {
			token,
			host: Arc::clone(self)
		})
	}
	
	fn call_method<Response>(&self, token: Token, method: Method) -> Response {
		let mut in_ = Some((token, method));
		let mut out = MaybeUninit::<Response>::uninit();

		self.send(
			WM_CALL_METHOD,
			(&raw mut in_).expose_provenance(),
			(&raw mut out).expose_provenance().cast_signed()
		);
		
		unsafe { out.assume_init() }
	}
	
	fn release_driver(&self, token: Token) {
		_ = self.post(WM_RELEASE_DRIVER, token.0);
	}

	fn send(&self, message: i32, w_param: usize, l_param: isize) {
		unsafe {
			SendMessageW(
				self.0.0,
				message as _,
				WPARAM(w_param),
				LPARAM(l_param)
			);
		}
	}
	
	fn post(&self, message: i32, w_param: usize) -> WinResult<()> {
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

impl Drop for Host {
	fn drop(&mut self) {
		_ = self.post(WM_CLOSE, 0);
	}
}

fn spawn_worker() -> oneshot::Receiver<SendHWND> {
	let (sender, receiver) = oneshot::channel::<SendHWND>();
		
	thread::spawn(move || {
		let window_handle = unsafe { CreateWindowExW(0, MAKEINTATOM(WINDOW_CLASS.0), None, 0, 0, 0, 0, 0, Some(HWND_MESSAGE), None, None, None) };
		sender.send(SendHWND(window_handle)).expect("receiver dropped prematurely");

		let mut context = WorkerContext::default();
		
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

const WM_CREATE_DRIVER : i32 = WM_APP;
const WM_CALL_METHOD   : i32 = WM_APP + 1;
const WM_RELEASE_DRIVER: i32 = WM_APP + 2;

unsafe extern "system" fn window_procedure(window_handle: HWND, message: u32, in_: WPARAM, out: LPARAM) -> LRESULT {
	// cannot get the context before matching the message since it might not exist yet (WM_NCCREATE, WM_CREATE, ...)
	match message as i32 {
		WM_CREATE_DRIVER  => get_context(window_handle).create_driver (in_, out),
		WM_CALL_METHOD    => get_context(window_handle).call_method   (in_, out),
		WM_RELEASE_DRIVER => get_context(window_handle).release_driver(in_),
		WM_DESTROY        => unsafe { PostQuitMessage(0) },
		_                 => return unsafe { DefWindowProcW(window_handle, message, in_, out) }
	}

	LRESULT::default()
}

fn get_context<'ctx>(window_handle: HWND) -> &'ctx mut WorkerContext {
	unsafe {
		GetWindowLongPtrW(window_handle, GWLP_USERDATA)
		.cast_unsigned()
		.pipe(ptr::with_exposed_provenance_mut::<WorkerContext>)
		.as_mut()
		.expect("invalid GWLP_USERDATA")
	}
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
struct WorkerContext {
	next_token_val: usize,
	drivers: HashMap<Token, driver::SafeHandle>
}

impl WorkerContext {
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

/// Identifies a hosted driver instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Token(usize);

#[derive(Debug)]
enum Method {
	Init { window_handle: Option<HWND> },
	Name,
	Version,
	LastError,
	Start,
	Stop,
	ChannelCounts,
	Latencies,
	BufferSize,
	CanSampleRate { sample_rate: sys::SampleRate },
	GetSampleRate,
	SetSampleRate { sample_rate: sys::SampleRate },
	ClockSources,
	SetClockSource { clock_source: sys::ClockSourceIndex },
	SamplePosition,
	ChannelInfo { channel_id: dto::ChannelId },
	DisposeBuffers,
	OpenControlPanel,
	OutputReady,
	
	CreateBuffers { channels: Vec<dto::ChannelId>, buffer_size: c_long, callbacks: *const sys::Callbacks},
	Future { selector: sys::FutureSelector, opt: *mut c_void }
}

/// A thread-safe proxy for a driver instance hosted in a [`Host`].
#[derive(Debug)]
pub struct Proxy {
	token: Token,
	host : Arc<Host>
}

impl Proxy {
	/// Returns the [`Host`] which `self` belongs to.
	#[must_use]
	pub const fn host(&self) -> &Arc<Host> {
		&self.host
	}
	
	fn call<Return>(&self, method: Method) -> Return {
		self.host.call_method(self.token, method)
	}
}

impl Drop for Proxy {
	fn drop(&mut self) {
		self.host.release_driver(self.token);
	}
}

impl Driver for Proxy {
	fn init              (&self, window_handle: Option<HWND>        ) -> bool                                    { self.call(Method::Init { window_handle }         ) }
	fn name              (&self                                     ) -> CString                                 { self.call(Method::Name                           ) }
	fn version           (&self                                     ) -> sys::DriverVersion                      { self.call(Method::Version                        ) }
	fn last_error        (&self                                     ) -> CString                                 { self.call(Method::LastError                      ) }
	fn start             (&self                                     ) -> crate::Result<()>                       { self.call(Method::Start                          ) }
	fn stop              (&self                                     ) -> crate::Result<()>                       { self.call(Method::Stop                           ) }
	fn channel_counts    (&self                                     ) -> crate::Result<dto::ChannelCounts>       { self.call(Method::ChannelCounts                  ) }
	fn latencies         (&self                                     ) -> crate::Result<dto::Latencies>           { self.call(Method::Latencies                      ) }
	fn buffer_size       (&self                                     ) -> crate::Result<dto::BufferSize>          { self.call(Method::BufferSize                     ) }
	fn can_sample_rate   (&self, sample_rate: sys::SampleRate       ) -> crate::Result<()>                       { self.call(Method::CanSampleRate { sample_rate }  ) }
	fn get_sample_rate   (&self                                     ) -> crate::Result<sys::SampleRate>          { self.call(Method::GetSampleRate                  ) }
	fn set_sample_rate   (&self, sample_rate: sys::SampleRate       ) -> crate::Result<()>                       { self.call(Method::SetSampleRate { sample_rate }  ) }
	fn clock_sources     (&self                                     ) -> crate::Result<Vec<sys::ClockSource>>    { self.call(Method::ClockSources                   ) }
	fn set_clock_source  (&self, clock_source: sys::ClockSourceIndex) -> crate::Result<()>                       { self.call(Method::SetClockSource { clock_source }) }
	fn sample_position   (&self                                     ) -> crate::Result<dto::SamplePosition>      { self.call(Method::SamplePosition                 ) }
	fn channel_info      (&self, channel_id: dto::ChannelId         ) -> crate::Result<dto::ChannelInfoResponse> { self.call(Method::ChannelInfo { channel_id }     ) }
	fn dispose_buffers   (&self                                     ) -> crate::Result<()>                       { self.call(Method::DisposeBuffers                 ) }
	fn open_control_panel(&self                                     ) -> crate::Result<()>                       { self.call(Method::OpenControlPanel               ) }
	fn output_ready      (&self                                     ) -> crate::Result<()>                       { self.call(Method::OutputReady                    ) }
	
	unsafe fn create_buffers(
		&self,
		channels: impl IntoIterator<Item=dto::ChannelId>,
		buffer_size: c_long,
		callbacks: *const sys::Callbacks
	) -> crate::Result<impl Iterator<Item=[*mut c_void; 2]>> {
		let method = Method::CreateBuffers {
			channels: channels.into_iter().collect(),
			buffer_size,
			callbacks
		};
		
		self
		.call::<crate::Result<Vec<[*mut c_void; 2]>>>(method)
		.map(Vec::into_iter)
	}

	fn future<T: AsioFuture>(&self, param: &mut T::Param) -> crate::Result<()> {
		self.call(Method::Future { selector: T::SELECTOR, opt: <*mut _>::cast(param) })
	}
}