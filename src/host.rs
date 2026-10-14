mod utils;
mod worker;

use std::ffi::{CString, c_long, c_void};
use std::fmt::Debug;
use std::mem::MaybeUninit;
use std::sync::Arc;
use tap::Pipe;
use windows_core::GUID;
use crate::{WinResult, dto, sys};
use crate::driver::Driver;
use crate::future::AsioFuture;
use crate::win::*;

use self::utils::SendHWND;

const WM_CREATE_DRIVER : i32 = WM_APP;
const WM_CALL_METHOD   : i32 = WM_APP + 1;
const WM_RELEASE_DRIVER: i32 = WM_APP + 2;

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

/// Identifies a hosted driver instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Token(usize);

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
		worker::spawn()
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