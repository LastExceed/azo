use std::ffi::{CString, c_long, c_void};
use std::num::NonZeroI32;
use std::{mem, ptr};
use crate::{WinResult, dto, sys};
use crate::dto::Granularity;
use crate::future::AsioFuture;
use crate::utils::{ResultCodeExt, cast_decoupled, cstring_from_bytes_until_nul};
use crate::win::{CLSCTX_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize, HWND, S_FALSE};
use sys::IIASIORedecl;
use tap::Pipe;
use windows_core::{GUID, HSTRING, IUnknown};

#[cfg(feature = "host")]
pub use crate::host::{Proxy, ExfiltratedHandle};

pub trait Driver {
	/// The spec unfortunately does not elaborate on the purpose of the parameter.
	#[must_use]
	fn init(&self, window_handle: Option<HWND>) -> bool;

	/// Usually (but not necessarily) the same as [`DriverMetadata::description`].
	#[must_use]
	fn name(&self) -> CString;
	
	/// Intended to be the major ASIO version (`2` since the release of ASIO 2.0 in 1999),
	/// but technically allowed to be higher by spec, and many report their own (independent) version this way.
	#[must_use]
	fn version(&self) -> sys::DriverVersion;
	
	/// Retrieves a message associated with the recentmost error.
	#[must_use]
	fn last_error(&self) -> CString;
	
	/// Drivers typically invoke the [`buffer_switch`](sys::Callbacks::buffer_switch) / [`buffer_switch_time_info`](sys::Callbacks::buffer_switch_time_info)
	/// callback 1+ times during (or immediately after) this function call to prime the output buffer(s).
	fn start(&self) -> crate::Result<()>;
	
	/// Halts the streaming.<br>
	/// The driver remains ready to resume via [`.start()`](Self::start).
	fn stop(&self) -> crate::Result<()>;
	
	/// Returns the number of channels in each direction.
	fn channel_counts(&self) -> crate::Result<dto::ChannelCounts>;
	
	/// Accounts for buffer size, assuming [`BufferSize::preferred`](dto::BufferSize::preferred)
	/// when called before [`.create_buffers()`](Self::create_buffers).
	fn latencies(&self) -> crate::Result<dto::Latencies>;
	
	/// Retrieves buffer size(s) supported by the driver.<br>
	/// These can depend on the current sample rate.
	fn buffer_size(&self) -> crate::Result<dto::BufferSize>;
	
	/// Checks whether the specified `sample_rate` is supported.
	fn can_sample_rate(&self, sample_rate: sys::SampleRate) -> crate::Result<()>;
	
	/// Returns the current sample rate.
	fn get_sample_rate(&self) -> crate::Result<sys::SampleRate>;
		
	/// 0 = external sync
	fn set_sample_rate(&self, sample_rate: sys::SampleRate) -> crate::Result<()>;
	
	/// Retrieves a list of all clock sources available to this driver.
	fn clock_sources(&self) -> crate::Result<Vec<sys::ClockSource>>;
	
	/// Selects a [`ClockSource`](sys::ClockSource), as enumerated via [`.clock_sources()`](Self::clock_sources)
	fn set_clock_source(&self, clock_source: sys::ClockSourceIndex) -> crate::Result<()>;
	
	/// Tells the driver to open its GUI
	fn sample_position(&self) -> crate::Result<dto::SamplePosition>;

	fn channel_info(&self, channel_id: dto::ChannelId) -> crate::Result<dto::ChannelInfoResponse>;

	fn dispose_buffers(&self) -> crate::Result<()>;
	
	/// Tells the driver to open its GUI
	fn open_control_panel(&self) -> crate::Result<()>;
	
	/// Tells the driver that the host is done processing output buffers.
	/// 
	/// This is *not* implicitly inferred from the return of [`Callbacks::buffer_switch`] / [`Callbacks::buffer_switch_time_info`],
	/// because it might have been called by a thread that doesn't allow processing within the callback.
	/// 
	/// # Caveats
	/// Devices without hardware DSP and no further internal buffering
	/// have no use for this signal, so their drivers might not support it,
	/// and instead return [`ResultCode::NOT_PRESENT`].
	/// This is not fatal, it just means that calls to this function can (and should) be skipped.
	/// Take care not to "error out" unnecessarily in this case.
	fn output_ready(&self) -> crate::Result<()>;
	
	/// # Safety
	/// * `callbacks` must outlive the created buffers.
	/// * Derefs of the returned buffer pointers must not.
	/// # Remarks
	/// This function is kept C-style because providing safe abstractions for it
	/// is very difficult to do without getting highly opinionated. (Help wanted!)
	unsafe fn create_buffers(
		&self,
		channels: impl IntoIterator<Item=dto::ChannelId>,
		buffer_size: c_long,
		callbacks: *const sys::Callbacks
	) -> crate::Result<impl Iterator<Item=[*mut c_void; 2]>>;
	
	/// A very unfortunate name. 
	/// This function actually has nothing to do with async code,
	/// it merely provides a mechanism for extending ASIO in the future.
	fn future<T: AsioFuture>(&self, param: &mut T::Param) -> crate::Result<()>;
}

/// Metadata of an ASIO driver, retrieved from the system registry via [`get_drivers`]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Metadata {
	pub clsid: GUID,
	pub description: HSTRING,
}

impl Metadata {
	/// Gathers the metadata of all ASIO drivers currently registered in the system.
	/// Malformed keys are skipped.
	/// 
	/// This is the "starting point" of this library.
	pub fn enumerate() -> WinResult<Vec<Self>> {
		let software_key = windows_registry::LOCAL_MACHINE.open("SOFTWARE\\ASIO")?;
			
		let drivers =
			software_key
			.keys()?
			.filter_map(|driver_key_name| {
				let driver_key = software_key.open(&driver_key_name).ok()?;
				Self::from_registry(&driver_key).ok()
			})
			.collect();
		
		Ok(drivers)
	}
	
	fn from_registry(key: &windows_registry::Key) -> WinResult<Self> {
		let clsid =
			key
			.get_string("clsid")?
			.trim_matches(['{', '}'])
			.try_into()?;
		
		let description =
			key
			.get_hstring("description")?;
		
		Ok(Self { clsid, description })
	}
}

/// A safe, [`Clone`]able handle to a driver instance.
/// This type is ! [`Send`] because the driver instance lives in a single-threaded COM apartment.
/// [`crate::utils::Host`] provides the necessary machinery to get around this limitation.
#[derive(Debug, PartialEq, Eq)]
pub struct SafeHandle(UnsafeHandle);

impl Clone for SafeHandle {
	fn clone(&self) -> Self {
		// increment the ref count
		let hresult = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED as _) };
		assert_eq!(hresult, S_FALSE, "COM should be initialized as STA");
		
		Self(self.0.clone())
	}
}

impl SafeHandle {
	pub fn new(clsid: &GUID) -> WinResult<Self> {
		let hresult = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED as _) };
		if !hresult.is_ok() {
			return Err(hresult.into());
		}
		
		// SAFETY:
		// COM is now initialized, and gets uninitialized in the `Drop` implementation of `Self`
		unsafe { UnsafeHandle::new(clsid) }?
    	.pipe(Self)
		.pipe(Ok)
	}
	
	/// # Safety
	/// The caller must ensure that the returned handle and derivatives do not outlive the driver instance.
	#[must_use]
	pub const unsafe fn as_unsafe(&self) -> &UnsafeHandle {
		&self.0
	}
}

impl Drop for SafeHandle {
	fn drop(&mut self) {
		unsafe { CoUninitialize(); }
	}
}

// can't use Deref as that would provide access to its `Clone` implementation without ever having to commit to the API contract
impl Driver for SafeHandle {
	fn init              (&self, window_handle: Option<HWND>        ) -> bool                                    { self.0.init              (window_handle) }
	fn name              (&self                                     ) -> CString                                 { self.0.name              (             ) }
	fn version           (&self                                     ) -> sys::DriverVersion                      { self.0.version           (             ) }
	fn last_error        (&self                                     ) -> CString                                 { self.0.last_error        (             ) }
	fn start             (&self                                     ) -> crate::Result<()>                       { self.0.start             (             ) }
	fn stop              (&self                                     ) -> crate::Result<()>                       { self.0.stop              (             ) }
	fn channel_counts    (&self                                     ) -> crate::Result<dto::ChannelCounts>       { self.0.channel_counts    (             ) }
	fn latencies         (&self                                     ) -> crate::Result<dto::Latencies>           { self.0.latencies         (             ) }
	fn buffer_size       (&self                                     ) -> crate::Result<dto::BufferSize>          { self.0.buffer_size       (             ) }
	fn can_sample_rate   (&self, sample_rate: sys::SampleRate       ) -> crate::Result<()>                       { self.0.can_sample_rate   (sample_rate  ) }
	fn get_sample_rate   (&self                                     ) -> crate::Result<sys::SampleRate>          { self.0.get_sample_rate   (             ) }
	fn set_sample_rate   (&self, sample_rate: sys::SampleRate       ) -> crate::Result<()>                       { self.0.set_sample_rate   (sample_rate  ) }
	fn clock_sources     (&self                                     ) -> crate::Result<Vec<sys::ClockSource>>    { self.0.clock_sources     (             ) }
	fn set_clock_source  (&self, clock_source: sys::ClockSourceIndex) -> crate::Result<()>                       { self.0.set_clock_source  (clock_source ) }
	fn sample_position   (&self                                     ) -> crate::Result<dto::SamplePosition>      { self.0.sample_position   (             ) }
	fn channel_info      (&self, channel_id: dto::ChannelId         ) -> crate::Result<dto::ChannelInfoResponse> { self.0.channel_info      (channel_id   ) }
	fn dispose_buffers   (&self                                     ) -> crate::Result<()>                       { self.0.dispose_buffers   (             ) }
	fn open_control_panel(&self                                     ) -> crate::Result<()>                       { self.0.open_control_panel(             ) }
	fn output_ready      (&self                                     ) -> crate::Result<()>                       { self.0.output_ready      (             ) }
	
	unsafe fn create_buffers(
		&self,
		channels   : impl IntoIterator<Item=dto::ChannelId>,
		buffer_size: c_long,
		callbacks  : *const azo_sys::Callbacks
	) -> crate::Result<impl Iterator<Item=[*mut c_void; 2]>> {
		unsafe { self.0.create_buffers(channels, buffer_size, callbacks) }
	}
	
	fn future<T: AsioFuture>(&self, param: &mut T::Param) -> crate::Result<()> {
		self.0.future::<T>(param)
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsafeHandle(pub IIASIORedecl);

impl UnsafeHandle {
	/// # Safety
	/// The caller must ensure that COM
	/// * is initialized on this thread
	/// * stays that way until this handle and all its clones got dropped
	pub unsafe fn new(guid: &GUID) -> WinResult<Self> {
		// Created as `IUnknown` because windows-rs binds this function in
		// a way where the IID is acquired from a trait-associated constant,
		// which is impossible to implement for `IIASIORedecl` (see its doc comment)
		let i_unknown: IUnknown = unsafe { CoCreateInstance(guid, None, CLSCTX_SERVER as _) }?;

		// The aforementioned binding limitation also applies to `.cast()`.
		// Luckily, the underlying `.query()` is public, which enables the following work-around:
		unsafe { cast_decoupled::<IIASIORedecl>(&i_unknown, guid) }
		.map(Self)
	}

	#[must_use]
	pub const fn from_raw(raw: IIASIORedecl) -> Self {
		Self(raw)
	}
}

impl Driver for UnsafeHandle {    
	fn init(&self, main_window_handle: Option<HWND>) -> bool {
		let sys_ref = main_window_handle.unwrap_or_default(); 

		unsafe { self.0.init(sys_ref.0) }
		.try_into()
		.unwrap_or(false)
	}

	fn name(&self) -> CString {
		let mut buf = [0_u8; 32];
		unsafe { self.0.get_driver_name(buf.as_mut_ptr()); }
		cstring_from_bytes_until_nul(&buf)
	}

	fn version(&self) -> sys::DriverVersion {
		unsafe { self.0.get_driver_version() }
	}

	fn last_error(&self) -> CString {
		let mut buf = [0_u8; 124];
		unsafe { self.0.get_error_message(buf.as_mut_ptr()); }
		cstring_from_bytes_until_nul(&buf)
	}
	
	fn start(&self) -> crate::Result<()> {
		unsafe { self.0.start() }
		.to_result()
	}
	fn stop(&self) -> crate::Result<()> {
		unsafe { self.0.stop() }
		.to_result()
	}

	fn channel_counts(&self) -> crate::Result<dto::ChannelCounts> {
		let mut counts = dto::ChannelCounts { in_: 0, out: 0 };
		unsafe { self.0.get_channels(&raw mut counts.in_, &raw mut counts.out) }
    	.to_result_with(|| counts)
	}

	fn latencies(&self) -> crate::Result<dto::Latencies> {
		let mut latencies = dto::Latencies { in_: 0, out: 0 };
		unsafe { self.0.get_latencies(&raw mut latencies.in_, &raw mut latencies.out) }
    	.to_result_with(|| latencies)
	}

	fn buffer_size(&self) -> crate::Result<dto::BufferSize> {
		let mut min         = -1;
		let mut max         = -2;
		let mut preferred   = -3;
		let mut granularity = -4;
		
		unsafe { self.0.get_buffer_size(&raw mut min, &raw mut max, &raw mut preferred, &raw mut granularity) }
    	.to_result()
    	.map(|()| dto::BufferSize {
			min,
			max,
			preferred,
			granularity: NonZeroI32::new(granularity).map(Granularity::from)
		})
	}

	fn can_sample_rate(&self, sample_rate: sys::SampleRate) -> crate::Result<()> {
		unsafe { self.0.can_sample_rate(sample_rate) }
    	.to_result()
	}
	
	fn get_sample_rate(&self) -> crate::Result<sys::SampleRate> {
		let mut sample_rate = f64::NAN;

		unsafe { self.0.get_sample_rate(&raw mut sample_rate) }
    	.to_result_with(|| sample_rate)
	}

	fn set_sample_rate(&self, sample_rate: sys::SampleRate) -> crate::Result<()> {
		unsafe { self.0.set_sample_rate(sample_rate) }
		.to_result()
	}

	#[expect(clippy::panic_in_result_fn, reason = "invalid driver behaviour")]
	fn clock_sources(&self) -> crate::Result<Vec<sys::ClockSource>> {
		let mut count = 1;
		let mut first = unsafe { mem::zeroed() };
		
		unsafe { self.0.get_clock_sources(&raw mut first, &raw mut count) }
		.to_result()?;
	
		match count {
			0   => Ok(vec![]),
			1   => Ok(vec![first]),
			2.. => {
				let mut all = vec![unsafe { mem::zeroed() }; count as _];
				unsafe { self.0.get_clock_sources(all.as_mut_ptr(), &raw mut count) }
				.to_result()
				.map(|()| all)
			}
			neg => panic!("driver reported negative number of clock sources ({neg})")
		}
	}

	/// Selects a [`ClockSource`](sys::ClockSource), as enumerated via [`.clock_sources()`](Self::clock_sources)
	fn set_clock_source(&self, clock_source: sys::ClockSourceIndex) -> crate::Result<()> {
		unsafe { self.0.set_clock_source(clock_source) }
    	.to_result()
	}

	fn sample_position(&self) -> crate::Result<dto::SamplePosition> {
		let mut position   = sys::Samples  ::default();
		let mut time_stamp = sys::TimeStamp::default();
		
		unsafe { self.0.get_sample_position(&raw mut position, &raw mut time_stamp) }
		.to_result_with(|| dto::SamplePosition {
			position  : position  .into(),
			time_stamp: time_stamp.into()
		})
	}

	fn channel_info(&self, channel_id: dto::ChannelId) -> crate::Result<dto::ChannelInfoResponse> {
		let mut info =
			sys::ChannelInfo {
				channel: channel_id.index,
				is_input: channel_id.input.into(),
				..unsafe { mem::zeroed() }
			};
		unsafe { self.0.get_channel_info(&raw mut info) }
    	.to_result_with(|| info.into())
	}

	unsafe fn create_buffers(
		&self,
		channels: impl IntoIterator<Item=dto::ChannelId>,
		buffer_size: c_long,
		callbacks: *const sys::Callbacks
	) -> crate::Result<impl Iterator<Item=[*mut c_void; 2]>> {
		let mut infos =
			channels
			.into_iter()
			.map(|dto::ChannelId { input, index }|
				sys::BufferInfo {
				    is_input: input.into(),
				    channel_num: index,
				    buffers: [ptr::null_mut(); 2]
				}
			)
			.collect::<Vec<_>>();
		
		unsafe { self.0.create_buffers(infos.as_mut_ptr(), infos.len() as _, buffer_size, callbacks.cast_mut()) }
    	.to_result_with(||
			infos
			.into_iter()
			.map(|info| info.buffers)
		)
	}

	fn dispose_buffers(&self) -> crate::Result<()> {
		unsafe { self.0.dispose_buffers() }
    	.to_result()
	}

	fn open_control_panel(&self) -> crate::Result<()> {
		unsafe { self.0.control_panel() }
		.to_result()
	}

	fn future<T: AsioFuture>(&self, param: &mut T::Param) -> crate::Result<()> {
		let selector = T::SELECTOR;
		let opt = ptr::from_mut(param).cast();
		
		unsafe { self.0.future(selector, opt) }
		.to_result()
	}
	
	fn output_ready(&self) -> crate::Result<()> {
		unsafe { self.0.output_ready() }
		.to_result()
	}
}