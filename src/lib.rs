pub mod com;
pub mod driver;
pub mod dto;
pub mod future;
pub mod utils;
pub mod win;
#[cfg(feature = "host")]
pub(crate) mod host;

use std::num::NonZeroI32;
use std::fmt;
use std::ffi::*;
use sys::ResultCode;
use self::future::AsioFuture;

pub use windows_core;
pub use azo_sys as sys;

pub trait Driver {
	/// The spec unfortunately does not elaborate on the purpose of the parameter.
	#[must_use]
	fn init(&self, window_handle: Option<win::HWND>) -> bool;
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
	fn start(&self) -> Result<()>;
	
	/// Halts the streaming.<br>
	/// The driver remains ready to resume via [`.start()`](Self::start).
	fn stop(&self) -> Result<()>;
	
	/// Returns the number of channels in each direction.
	fn channel_counts(&self) -> Result<dto::ChannelCounts>;
	
	/// Accounts for buffer size, assuming [`BufferSize::preferred`](dto::BufferSize::preferred)
	/// when called before [`.create_buffers()`](Self::create_buffers).
	fn latencies(&self) -> Result<dto::Latencies>;
	
	/// Retrieves buffer size(s) supported by the driver.<br>
	/// These can depend on the current sample rate.
	fn buffer_size(&self) -> Result<dto::BufferSize>;
	
	/// Checks whether the specified `sample_rate` is supported.
	fn can_sample_rate(&self, sample_rate: sys::SampleRate) -> Result<()>;
	
	/// Returns the current sample rate.
	fn get_sample_rate(&self) -> Result<sys::SampleRate>;
		
	/// 0 = external sync
	fn set_sample_rate(&self, sample_rate: sys::SampleRate) -> Result<()>;
	
	/// Retrieves a list of all clock sources available to this driver.
	fn clock_sources(&self) -> Result<Vec<sys::ClockSource>>;
	
	/// Selects a [`ClockSource`](sys::ClockSource), as enumerated via [`.clock_sources()`](Self::clock_sources)
	fn set_clock_source(&self, clock_source: sys::ClockSourceIndex) -> Result<()>;
	
	/// Tells the driver to open its GUI
	fn sample_position(&self) -> Result<dto::SamplePosition>;
	fn channel_info(&self, channel_id: dto::ChannelId) -> Result<dto::ChannelInfoResponse>;
	fn dispose_buffers(&self) -> Result<()>;
	
	/// Tells the driver to open its GUI
	fn open_control_panel(&self) -> Result<()>;
	
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
	fn output_ready(&self) -> Result<()>;
	
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
	) -> Result<impl Iterator<Item=[*mut c_void; 2]>>;
	
	/// A very unfortunate name. 
	/// This function actually has nothing to do with async code,
	/// it merely provides a mechanism for extending ASIO in the future.
	fn future<T: AsioFuture>(&self, param: &mut T::Param) -> Result<()>;
}


#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Error(NonZeroI32);

impl Error {
    /// guaranteed to never be [`ResultCode::OK`] / [`ResultCode::SUCCESS`]
    #[must_use]
    pub const fn code(&self) -> ResultCode {
        ResultCode(self.0.get())
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.code().fmt(f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.code().fmt(f)
    }
}

#[expect(clippy::absolute_paths, reason = "name collision")]
impl std::error::Error for Error {}

#[expect(clippy::absolute_paths, reason = "name collision")]
pub type Result<T> = std::result::Result<T, Error>;