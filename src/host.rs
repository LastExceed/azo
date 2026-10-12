use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::ffi::{CString, c_long, c_void};
use std::fmt::Debug;
use std::os::windows::io::AsRawHandle;
use std::sync::{Arc, Once, mpsc};
use std::{ptr, thread};
use std::time::Duration;
use tap::Pipe;
use windows_core::{GUID, PCWSTR, WIN32_ERROR, w};
use crate::utils::ResultCodeExt;
use crate::{WinResult, driver, dto, sys};
use crate::driver::Driver;
use crate::future::AsioFuture;
use crate::win::*;

/// The window class of the apartment's message-only window.
const CLASS_NAME: PCWSTR = w!("azo-host");

/// A COM STA to create driver instances in, hosted by a dedicated thread driving a message pump.
/// Provides a mechanism for funneling driver interactions through the apartment owner
/// via [`Proxy`] structs, similar to COMs marshaling (which ASIO unfortunately does not support).
/// 
/// > ...what?
/// 
/// Too much technical jargon? Then [`Host`] is exactly what you need: An abstraction over a bunch of
/// arcane machinery required for correctness, which consumers of [`azo`](crate) should not have to worry about.
///
/// Commands reach the apartment as messages to a message-only window rather than as thread
/// messages: a modal loop on the apartment's thread (a driver's control panel dialog,
/// `MessageBox`) dispatches window messages but drops thread messages, so a command sent
/// while a panel is open would never be answered. Such a command runs inside the modal loop,
/// re-entrantly, as a window-based host's would.
#[derive(Debug)]
pub struct Host {
	thread_id: u32,
	/// The message-only window's handle (an `HWND`, which is not [`Send`]).
	window: usize
}

impl Host {
	pub fn new() -> Arc<Self> {
		let (sender, receiver) = mpsc::channel();
		let thread_id = thread::spawn(move || WorkerContext::host_sta(&sender))
		.as_raw_handle()
		.pipe(HANDLE)
		.pipe(|handle| unsafe { GetThreadId(handle) });
		let window = receiver.recv().expect("the apartment thread should create its window");
		assert_ne!(window, 0, "creating the apartment's message-only window failed");
		Arc::new(Self { thread_id, window })
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
				PostMessageW(
					Some(HWND(self.window as _)),
					WM_APP as _,
					WPARAM(in_ as _),
					LPARAM(out as _)
				)
			};
			
			if !success.as_bool() {
				assert_eq!(WIN32_ERROR::from_thread(), WIN32_ERROR(ERROR_NOT_ENOUGH_QUOTA as _), "undocumented error occurred");
				// the queue is full: wait for the apartment to catch up.
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

/// The apartment's state, on its own thread. Commands may run re-entrantly (from a modal loop
/// inside a driver call), so no borrow is held across a call into a driver.
#[derive(Debug, Default)]
struct WorkerContext {
	next_token_val: Cell<usize>,
	// `Rc`, not a clone of the handle: a COM clone is an `AddRef`/`Release` pair, and some
	// drivers (wineasio) tear their instance down on every `Release`, not only the last.
	drivers: RefCell<HashMap<Token, Rc<driver::SafeHandle>>>
}

thread_local! {
	static WORKER: WorkerContext = WorkerContext::default();
}

static REGISTER_CLASS: Once = Once::new();

unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
	if message == WM_APP as u32 {
		unsafe { WorkerContext::handle_command(wparam, lparam); }
		return LRESULT(0);
	}
	unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

impl WorkerContext {
	fn host_sta(window: &mpsc::Sender<usize>) {
		let init_result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED as _) };
		assert_eq!(init_result, S_OK, "COM STA initialization on a fresh thread should be infallible");

		let handle = Self::create_window();
		_ = window.send(handle.0 as usize);
		if !handle.0.is_null() {
			Self::pump();
			_ = unsafe { DestroyWindow(handle) };
		}
		
		// Release the drivers while COM is still initialised.
		let drivers = WORKER.with(|w| w.drivers.take());
		drop(drivers);
		unsafe { CoUninitialize(); }
	}
	
	fn create_window() -> HWND {
		let instance = unsafe { GetModuleHandleW(PCWSTR::null()) };
		REGISTER_CLASS.call_once(|| {
			let class = WNDCLASSEXW {
				cbSize: size_of::<WNDCLASSEXW>() as u32,
				lpfnWndProc: Some(window_proc),
				hInstance: instance,
				lpszClassName: CLASS_NAME,
				..Default::default()
			};
			// A failure shows as the window's creation failing.
			_ = unsafe { RegisterClassExW(&raw const class) };
		});
		unsafe {
			CreateWindowExW(0, CLASS_NAME, PCWSTR::null(), 0, 0, 0, 0, 0, Some(HWND_MESSAGE), None, Some(instance), None)
		}
	}

	fn pump() {
		let mut msg = MSG::default();
		loop {
			// contrary to what the return type suggests, `GetMessageW` is fallible
			let res = unsafe { GetMessageW(&raw mut msg, None, 0, 0) };
			match res.0 {
				-1 => (), // ignore invalid messages
				0 => break, // WM_QUIT received
				_ => unsafe { DispatchMessageW(&raw const msg); }
			}
		}
	}

	unsafe fn handle_command(wparam: WPARAM, lparam: LPARAM) {
		let command = unsafe {
			*Box::from_raw(wparam.0 as *mut Command)
		};
		let out = lparam.0 as *mut ();
		
		match command {
			Command::Create(guid) => {
				let result = Self::try_create_driver(&guid);
				unsafe { respond(out, result) };
			}
			
			Command::CallMethod(token, method) => {
				// A second `Rc` keeps the driver alive through the call even if a re-entrant
				// `Release` removes it from the map meanwhile; the driver's own reference
				// count is not touched.
				let driver = WORKER
				.with(|w| w.drivers.borrow().get(&token).cloned())
				.expect("a proxy's driver lives until the proxy is dropped");
				
				match method {
					Method::Init { window_handle }          => driver.init              (window_handle).pipe(|ret| unsafe { respond(out, ret) }),
					Method::Name                            => driver.name              (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::Version                         => driver.version           (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::LastError                       => driver.last_error        (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::Start                           => driver.start             (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::Stop                            => driver.stop              (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::ChannelCounts                   => driver.channel_counts    (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::Latencies                       => driver.latencies         (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::BufferSize                      => driver.buffer_size       (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::CanSampleRate { sample_rate }   => driver.can_sample_rate   (sample_rate  ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::GetSampleRate                   => driver.get_sample_rate   (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::SetSampleRate { sample_rate }   => driver.set_sample_rate   (sample_rate  ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::ClockSources                    => driver.clock_sources     (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::SetClockSource { clock_source } => driver.set_clock_source  (clock_source ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::SamplePosition                  => driver.sample_position   (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::ChannelInfo { channel_id }      => driver.channel_info      (channel_id   ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::DisposeBuffers                  => driver.dispose_buffers   (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::OpenControlPanel                => driver.open_control_panel(             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::OutputReady                     => driver.output_ready      (             ).pipe(|ret| unsafe { respond(out, ret) }),
					Method::CreateBuffers { channels, buffer_size, callbacks } => unsafe {
						let ret = driver.create_buffers(channels, buffer_size, callbacks).map(Iterator::collect::<Vec<_>>);
						respond(out, ret);
					}
					Method::Future { selector, opt } => unsafe {
						driver
						.as_unsafe()
						.0
						.future(selector, opt)
						.to_result()
						.pipe(|ret| respond(out, ret));
					},
				}
			}
			
			Command::Release(token) => {
				// Taken out first: the driver's release may pump messages, re-entering here.
				let driver = WORKER.with(|w| w.drivers.borrow_mut().remove(&token));
				drop(driver);
			}
		}
	}

	fn try_create_driver(guid: &GUID) -> WinResult<Token> {
		let driver = driver::SafeHandle::new(guid)?;
		WORKER.with(|w| {
			let token = Token(w.next_token_val.get());
			w.next_token_val.set(token.0 + 1);
			w.drivers.borrow_mut().insert(token, Rc::new(driver));
			Ok(token)
		})
	}
}

/// Answers a command through the sender its poster boxed into `out`.
///
/// # Safety
/// `out` is the `oneshot::Sender<Response>` [`Host::command_with_response`] boxed for this
/// command.
unsafe fn respond<Response>(out: *mut (), response: Response) {
	_ = unsafe { Box::from_raw(out.cast::<oneshot::Sender<Response>>()) }.send(response);
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