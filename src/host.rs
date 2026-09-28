use std::collections::HashMap;
use std::ffi::{CString, c_long, c_void};
use std::fmt::Debug;
use std::ops::Deref;
use std::os::windows::io::AsRawHandle;
use std::sync::Arc;
use std::{ptr, thread};
use std::time::Duration;
use tap::Pipe;
use windows_core::{GUID, WIN32_ERROR};
use crate::utils::ResultCodeExt;
use crate::{WinResult, driver, dto, sys};
use crate::driver::Driver;
use crate::future::AsioFuture;
use crate::win::*;

const POST_MESSAGE_ERRORS: [WIN32_ERROR; 2] = [
	WIN32_ERROR(ERROR_INVALID_THREAD_ID as _),
	WIN32_ERROR(ERROR_NOT_ENOUGH_QUOTA  as _)
];

/// A COM STA to create driver instances in, hosted by a dedicated thread driving a message pump.
/// Provides a mechanism for funneling driver interactions through the apartment owner
/// via [`Proxy`] structs, similar to COMs marshaling (which ASIO unfortunately does not support).
/// 
/// > ...what?
/// 
/// Too much technical jargon? Then [`Host`] is exactly what you need: An abstraction over a bunch of
/// arcane machinery required for correctness, which consumers of [`azo`](crate) should not have to worry about.
#[derive(Debug)]
pub struct Host {
	thread_id: u32
}

impl Host {
	pub fn new() -> Arc<Self> {
		thread::spawn(WorkerContext::host_sta)
		.as_raw_handle()
		.pipe(HANDLE)
		.pipe(|handle| unsafe { GetThreadId(handle) })
		.pipe(|thread_id| Self { thread_id })
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
		self.post_message(command, ptr::null_mut());
	}
	
	fn command_with_response<Response>(&self, command: Command) -> Response {
		let (sender, receiver) = oneshot::channel::<Response>();
		
		let out_param = Box::new(sender).pipe(Box::into_raw);
		self.post_message(command, out_param.cast());
		
		receiver.recv().expect("worker shouldn't drop the sender")
	}
	
	fn post_message(&self, command: Command, out: *mut ()) {
		let in_ = Box::new(command).pipe(Box::into_raw);
		
		loop {
			let success = unsafe {
				PostThreadMessageW(
					self.thread_id,
					WM_USER as _,
					WPARAM(in_ as _),
					LPARAM(out as _)
				)
			};
			
			if !success.as_bool() {
				assert!(POST_MESSAGE_ERRORS.contains(&WIN32_ERROR::from_thread()), "undocumented error occurred");
				// the worker thread is still initializing (or overworked).
				// In either case, just wait and try again (should only take a few attempts).
				thread::sleep(Duration::from_nanos(1)); // sleep for as little as possible (1ms by default on win10+)
				continue;
			}
			
			break;
		}
	}
}

impl Drop for Host {
	fn drop(&mut self) {
		_ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT as _, WPARAM(0), LPARAM(0)) };
	}
}

#[derive(Debug, Default)]
struct WorkerContext {
	next_token_val: usize,
	drivers: HashMap<Token, driver::SafeHandle>,
	current_msg: MSG
}

impl WorkerContext {
	fn host_sta() {
		let init_result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED as _) };
		assert_eq!(init_result, S_OK, "COM STA initialization on a fresh thread should be infallible");

		Self::default().pump();
		
		unsafe { CoUninitialize(); }
	}

	fn pump(&mut self) {
		loop {
			let res = unsafe { GetMessageW(&raw mut self.current_msg, None, 0, 0) };
			if !res.as_bool() {
				break;
			}
			
			if self.current_msg.message == WM_USER as u32 {
				unsafe { self.handle_command(); }
			}
			
			unsafe { DispatchMessageW(&raw const self.current_msg); }
		}
	}

	unsafe fn handle_command(&mut self) {
		let command = unsafe {
			*Box::from_raw(self.current_msg.wParam.0 as *mut Command)
		};
		
		match command {
			Command::Create(guid) =>
				self
				.create_driver(&guid)
        		.pipe(|result| self.respond(result)),
			
			Command::CallMethod(token, method) =>
				self
				.drivers
				[&token]
				.pipe_ref(|driver| self.call_method(driver, method)),
			
			Command::Exfiltrate(token) =>
				self
				.drivers
				[&token]
				.pipe_ref(|sh| unsafe { sh.as_unsafe() })
				.clone()
				.pipe(ExfiltratedHandle)
				.pipe(|driver| self.respond(driver)),
			
			Command::Release(token) => {
				// no need to worry about any potential exfiltrations,
				// as COM interface pointers are themselves reference counted.
				_ = self.drivers.remove(&token);
			}
		}
	}
	
	fn call_method(&self, driver: &driver::SafeHandle, method: Method) {
		match method {
			Method::Init { window_handle }          => driver.init              (window_handle).pipe(|ret| self.respond(ret)),
			Method::Name                            => driver.name              (             ).pipe(|ret| self.respond(ret)),
			Method::Version                         => driver.version           (             ).pipe(|ret| self.respond(ret)),
			Method::LastError                       => driver.last_error        (             ).pipe(|ret| self.respond(ret)),
			Method::Start                           => driver.start             (             ).pipe(|ret| self.respond(ret)),
			Method::Stop                            => driver.stop              (             ).pipe(|ret| self.respond(ret)),
			Method::ChannelCounts                   => driver.channel_counts    (             ).pipe(|ret| self.respond(ret)),
			Method::Latencies                       => driver.latencies         (             ).pipe(|ret| self.respond(ret)),
			Method::BufferSize                      => driver.buffer_size       (             ).pipe(|ret| self.respond(ret)),
			Method::CanSampleRate { sample_rate }   => driver.can_sample_rate   (sample_rate  ).pipe(|ret| self.respond(ret)),
			Method::GetSampleRate                   => driver.get_sample_rate   (             ).pipe(|ret| self.respond(ret)),
			Method::SetSampleRate { sample_rate }   => driver.set_sample_rate   (sample_rate  ).pipe(|ret| self.respond(ret)),
			Method::ClockSources                    => driver.clock_sources     (             ).pipe(|ret| self.respond(ret)),
			Method::SetClockSource { clock_source } => driver.set_clock_source  (clock_source ).pipe(|ret| self.respond(ret)),
			Method::SamplePosition                  => driver.sample_position   (             ).pipe(|ret| self.respond(ret)),
			Method::ChannelInfo { channel_id }      => driver.channel_info      (channel_id   ).pipe(|ret| self.respond(ret)),
			Method::DisposeBuffers                  => driver.dispose_buffers   (             ).pipe(|ret| self.respond(ret)),
			Method::OpenControlPanel                => driver.open_control_panel(             ).pipe(|ret| self.respond(ret)),
			Method::OutputReady                     => driver.output_ready      (             ).pipe(|ret| self.respond(ret)),
			
			Method::CreateBuffers {
				channels,
				buffer_size,
				callbacks
			} => unsafe {
				let ret = driver.create_buffers(channels, buffer_size, callbacks).map(Iterator::collect::<Vec<_>>);
				self.respond(ret);
			}
			
			Method::Future { selector, opt } => unsafe {
				driver
				.as_unsafe()
				.0
				.future(selector, opt)
				.to_result()
				.pipe(|ret| self.respond(ret));
			},
		}
	}

	fn create_driver(&mut self, guid: &GUID) -> WinResult<Token> {
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

	fn respond<Response>(&self, response: Response) {
		_ = unsafe {
			(self.current_msg.lParam.0 as *mut oneshot::Sender<Response>)
			.as_mut()
			.unwrap()
			.pipe(|ptr| Box::from_raw(ptr))
			.send(response)
		};
	}
}

#[derive(Debug)]
enum Command {
	Create(GUID),
	CallMethod(Token, Method),
	Exfiltrate(Token),
	Release(Token)
}

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

#[cfg(feature = "host")]
#[derive(Debug)]
pub struct Proxy {
	token: Token,
	host : Arc<Host>
}

#[cfg(feature = "host")]
impl Proxy {
	/// Returns the host that `self` belongs to.
	#[must_use]
	pub const fn host(&self) -> &Arc<Host> {
		&self.host
	}
	
	fn call<Return>(&self, method: Method) -> Return {
		self.host.command_with_response(Command::CallMethod(self.token, method))
	}

	/// Exfiltrates a direct handle to the driver out of its COM apartment.
	/// 
	/// ## WARNING: This violates COMs threading rules!
	/// 
	/// This function primarily exists because of [`Driver::sample_position`] being essentially impossible to use correctly:
	/// Unless the driver supports the [`BufferSwitchTimeInfo`](crate::sys::BufferSwitchTimeInfo) callback, [`Driver::sample_position`]
	/// is the only way to retrieve timing information, and the authors of ASIO suggest calling it from within [`BufferSwitch`](crate::sys::BufferSwitch).
	/// However, doing so actually violates COMs threading rules, because the driver instance resides in a single threaded apartment,
	/// and the thread which invokes the callback is not the apartment owner. On the other hand, delegating the function call to the
	/// apartment owner is not feasible, as the latency and jitter introduced by thread synchronization would render the returned timing information meaningless,
	/// and more importantly, it can easily exceed the entire duration of a small buffer multiple times over.
	/// 
	/// Realistically though, ASIO drivers should be aware of this problem, and be able to handle calls to [`Driver::sample_position`] from the callback thread just fine.
	/// Still, always prefer timing information provided in [`BufferSwitchTimeInfo`](crate::sys::BufferSwitchTimeInfo) and avoid using exfiltration for any other purpose
	/// whenever possible.
	/// 
	/// # Safety
	/// The returned handle must not outlive [`Self::host`].
	#[must_use]
	pub unsafe fn exfiltrate(&self) -> ExfiltratedHandle {
		self.host.command_with_response(Command::Exfiltrate(self.token))
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExfiltratedHandle(driver::UnsafeHandle);

// SAFETY:
// Exfiltration already broke COMs threading rules, so this is fair game now.
unsafe impl Send for ExfiltratedHandle {}
unsafe impl Sync for ExfiltratedHandle {}

impl Deref for ExfiltratedHandle {
	type Target = driver::UnsafeHandle;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
