use std::collections::HashMap;
use std::ffi::{CString, c_long, c_void};
use std::fmt::Debug;
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
	pub fn create_driver(self: &Arc<Self>, guid: GUID) -> WinResult<Proxy> {
		self
		.command_with_response::<WinResult<Token>>(Command::Create(guid))
		.map(|token| Proxy { token, host: Arc::clone(self) })
	}
	
	fn command(&self, command: Command) {
		_ = self.send_message(command);
	}
	
	fn command_with_response<Response>(&self, command: Command) -> Response {
		let ptr = self.send_message(command);
		unsafe { restore_erased(ptr) }
	}
	
	fn send_message(&self, command: Command) -> *mut () {
		let in_ = Box::new(command).pipe(Box::into_raw);

		unsafe {
			SendMessageW(
				self.0.0,
				WM_APP as _,
				WPARAM(in_ as _),
				LPARAM::default()
			)
		}
		.0 as *mut ()
	}
}

impl Drop for Host {
	fn drop(&mut self) {
		_ = unsafe {
			SendMessageW(
				self.0.0,
				WM_CLOSE as _,
				WPARAM::default(),
				LPARAM::default()
			)
		};
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

unsafe extern "system" fn window_procedure(window_handle: HWND, message: u32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
	if message as i32 != WM_APP {
		return unsafe { DefWindowProcW(window_handle, message, w_param, l_param) };
	}
	
	let command =
		w_param
		.0
		.pipe(ptr::with_exposed_provenance_mut::<Command>)
		.pipe(|ptr| unsafe { Box::from_raw(ptr) });

	unsafe {
		GetWindowLongPtrW(window_handle, GWLP_USERDATA)
		.cast_unsigned()
		.pipe(ptr::with_exposed_provenance_mut::<WorkerContext>)
		.as_mut()
		.expect("invalid GWLP_USERDATA")
		.handle_command(*command)
		.expose_provenance()
		.cast_signed()
		.pipe(LRESULT)
	}
}

fn erase_type<T>(value: T) -> *mut () {
	value
    .pipe(Box::new)
	.pipe(Box::into_raw)
	.cast()
}

unsafe fn restore_erased<T>(raw_box: *mut ()) -> T {
	unsafe { *Box::from_raw(raw_box.cast()) }
}

#[derive(Debug, Default)]
struct WorkerContext {
	next_token_val: usize,
	drivers: HashMap<Token, driver::SafeHandle>
}

impl WorkerContext {
	fn handle_command(&mut self, command: Command) -> *mut () {
		match command {
			Command::Create(guid) => {
				self
				.try_create_driver(&guid)
				.pipe(erase_type)
			}
			
			Command::CallMethod(token, method) => {
				let driver = &self.drivers[&token];
				
				match method {
					Method::Init { window_handle }          => driver.init              (window_handle).pipe(erase_type),
					Method::Name                            => driver.name              (             ).pipe(erase_type),
					Method::Version                         => driver.version           (             ).pipe(erase_type),
					Method::LastError                       => driver.last_error        (             ).pipe(erase_type),
					Method::Start                           => driver.start             (             ).pipe(erase_type),
					Method::Stop                            => driver.stop              (             ).pipe(erase_type),
					Method::ChannelCounts                   => driver.channel_counts    (             ).pipe(erase_type),
					Method::Latencies                       => driver.latencies         (             ).pipe(erase_type),
					Method::BufferSize                      => driver.buffer_size       (             ).pipe(erase_type),
					Method::CanSampleRate { sample_rate }   => driver.can_sample_rate   (sample_rate  ).pipe(erase_type),
					Method::GetSampleRate                   => driver.get_sample_rate   (             ).pipe(erase_type),
					Method::SetSampleRate { sample_rate }   => driver.set_sample_rate   (sample_rate  ).pipe(erase_type),
					Method::ClockSources                    => driver.clock_sources     (             ).pipe(erase_type),
					Method::SetClockSource { clock_source } => driver.set_clock_source  (clock_source ).pipe(erase_type),
					Method::SamplePosition                  => driver.sample_position   (             ).pipe(erase_type),
					Method::ChannelInfo { channel_id }      => driver.channel_info      (channel_id   ).pipe(erase_type),
					Method::DisposeBuffers                  => driver.dispose_buffers   (             ).pipe(erase_type),
					Method::OpenControlPanel                => driver.open_control_panel(             ).pipe(erase_type),
					Method::OutputReady                     => driver.output_ready      (             ).pipe(erase_type),
					Method::CreateBuffers { channels, buffer_size, callbacks } => unsafe {
						driver
						.create_buffers(channels, buffer_size, callbacks)
						.map(Iterator::collect::<Vec<_>>)
						.pipe(erase_type)
					}
					Method::Future { selector, opt } => unsafe {
						driver
						.as_unsafe()
						.0
						.future(selector, opt)
						.to_result()
						.pipe(erase_type)
					}
				}
			}
			
			Command::Release(token) => {
				drop(self.drivers.remove(&token));
				ptr::null_mut()
			}
		}
	}

	fn try_create_driver(&mut self, guid: &GUID) -> WinResult<Token> {
		let driver = driver::SafeHandle::new(guid)?;
		let token = self.create_token();

		self.drivers.insert(token, driver);
		Ok(token)
	}
	
	const fn create_token(&mut self) -> Token {
		let token = Token(self.next_token_val);
		self.next_token_val += 1;
		token
	}
}

#[derive(Debug)]
enum Command {
	Create(GUID),
	CallMethod(Token, Method),
	Release(Token)
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
		self.host.command_with_response(Command::CallMethod(self.token, method))
	}
}

impl Drop for Proxy {
	fn drop(&mut self) {
		self.host.command(Command::Release(self.token));
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