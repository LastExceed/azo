pub mod dto;
pub mod future;
mod utils;
#[expect(
    dead_code,
    non_snake_case,
    unreachable_pub,
    unused_results,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction,
    clippy::style,
    reason = "generated"
)]
mod windows_bindings {
    include!(concat!(env!("OUT_DIR"), "/windows_bindgen_out.rs"));
}

use std::num::NonZeroI32;
use std::{fmt, mem, ptr};
use std::ffi::*;
use sys::{ErrorCode, IIASIORedecl};
use windows_core::{GUID, HSTRING};
use self::future::Future;
use self::utils::*;

pub use self::windows_bindings::{HWND, HANDLE, COINIT, COINIT_APARTMENTTHREADED};
pub use azo_sys as sys;

type WinResult<T> = windows_core::Result<T>;

/// Enumerates all available ASIO drivers
pub fn discover_drivers() -> WinResult<Vec<DriverMetadata>> {
    let software_key = windows_registry::LOCAL_MACHINE.open("SOFTWARE\\ASIO")?;
        
    software_key
    .keys()?
    .map(|driver_key_name| {
        let driver_key = software_key.open(&driver_key_name)?;
        DriverMetadata::from_registry(&driver_key)
    })
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DriverMetadata {
    pub clsid: GUID,
    pub description: HSTRING,
}

impl DriverMetadata {
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
    
    pub fn create_instance(&self) -> WinResult<Driver> {
        let com = COM::new(COINIT_APARTMENTTHREADED)?;
        let interface = com.create_driver_instance(&raw const self.clsid)?;

        Ok(Driver(interface, com))
    }
}

#[derive(Debug)]
pub struct Driver(IIASIORedecl, COM);
impl Driver {
    #[must_use]
    pub fn init(&self, main_window_handle: Option<HWND>) -> bool {
        let sys_ref = main_window_handle.unwrap_or_default(); 

        unsafe { self.0.init(sys_ref.0) }
        .try_into()
        .unwrap_or(false)
    }
    
    #[must_use]
    pub fn name(&self) -> CString {
        let mut buf = [0_u8; 32];
        unsafe { self.0.get_driver_name(buf.as_mut_ptr()); }
        cstring_from_bytes_until_nul(&buf)
    }

    #[must_use]
    pub fn version(&self) -> sys::DriverVersion {
        unsafe { self.0.get_driver_version() }
    }
    
    #[must_use]
    pub fn last_error(&self) -> CString {
        let mut buf = [0_u8; 124];
        unsafe { self.0.get_error_message(buf.as_mut_ptr()); }
        cstring_from_bytes_until_nul(&buf)
    }
    
    pub fn start(&self) -> Result<()> {
        let code = unsafe { self.0.start() };
        create_result((), code)
    }
    
    pub fn stop(&self) -> Result<()> {
        let code = unsafe { self.0.stop() };
        create_result((), code)
    }

	pub fn channel_counts(&self) -> Result<dto::ChannelCounts> {
        let mut counts = dto::ChannelCounts { in_: 0, out: 0 };
        let code = unsafe { self.0.get_channels(&raw mut counts.in_, &raw mut counts.out) };
        create_result(counts, code)
    }

    pub fn latencies(&self) -> Result<dto::Latencies> {
        let mut latencies = dto::Latencies { in_: 0, out: 0 };
        let code = unsafe { self.0.get_latencies(&raw mut latencies.in_, &raw mut latencies.out) };
        create_result(latencies, code)
    }

    #[expect(clippy::panic_in_result_fn, reason = "invalid driver behaviour")]
    pub fn buffer_size(&self) -> Result<dto::BufferSize> {
        use core::range::*; // new range types from Rust 1.96
        let mut range = RangeInclusive::from(-2..=-1); // this `.into()` will become obsolete when the new range types become the default in Rust 2027 edition
        let mut preferred   = -3;
        let mut granularity = -4;
        let code = unsafe { self.0.get_buffer_size(&raw mut range.start, &raw mut range.last, &raw mut preferred, &raw mut granularity) };
        create_result((), code)?;

        let has_fixed_size = range.start == range.last;
        let maybe_non_zero = NonZeroI32::new(granularity);
        
        assert_eq!(has_fixed_size, maybe_non_zero.is_none(), "invalid driver behaviour: granularity {granularity} is not compatible with range {range:?}");
        assert!(range.contains(&preferred), "invalid driver behaviour: preferred buffer size {preferred} is not within supported range {range:?}");
        
        let buffer_size =
            dto::BufferSize {
                preferred,
                range: maybe_non_zero.map(|non_zero| (range, non_zero.into()))
            };

        Ok(buffer_size)
    }

	pub fn can_sample_rate(&self, sample_rate: sys::SampleRate) -> Result<()> {
        let code = unsafe { self.0.can_sample_rate(sample_rate) };
        create_result((), code)
    }
    
    pub fn get_sample_rate(&self) -> Result<sys::SampleRate> {
        let mut sample_rate = f64::NAN;
        let code = unsafe { self.0.get_sample_rate(&raw mut sample_rate) };
        create_result(sample_rate, code)
    }
    
    pub fn set_sample_rate(&self, sample_rate: sys::SampleRate) -> Result<()> {
        let code = unsafe { self.0.set_sample_rate(sample_rate) };
        create_result((), code)
    }
    
    #[expect(clippy::panic_in_result_fn, reason = "invalid driver behaviour")]
	pub fn clock_sources(&self) -> Result<Vec<sys::ClockSource>> {
        let mut count = 1;
        let mut first = unsafe { mem::zeroed() };
        
        let code = unsafe { self.0.get_clock_sources(&raw mut first, &raw mut count) };
        create_result((), code)?;
    
        match count {
            0   => Ok(Vec::new()),
            1   => Ok([first].into()),
            2.. => {
                let mut all = vec![unsafe { mem::zeroed() }; count as _];
                let mut count2 = 0;
                
                let code2 = unsafe { self.0.get_clock_sources(all.as_mut_ptr(), &raw mut count2) };
                assert_eq!(count, count2, "reported number of clock sources changed ({count} -> {count2})");
                create_result(all, code2)
            }
            neg => panic!("driver reported negative number of clock sources ({neg})")
        }
    }

	pub fn set_clock_source(&self, clock_source: sys::ClockSourceIndex) -> Result<()> {
        let code = unsafe { self.0.set_clock_source(clock_source) };
        create_result((), code)
    }

	pub fn sample_position(&self) -> Result<dto::SamplePosition> {
        let mut sample_pos = dto::SamplePosition { position: 0, time_stamp: 0 };
        let code = unsafe { self.0.get_sample_position(&raw mut sample_pos.position, &raw mut sample_pos.time_stamp) };
        create_result(sample_pos, code)
    }

	pub fn channel_info(&self, channel_id: dto::ChannelId) -> Result<dto::ChannelInfoResponse> {
        let mut info =
            sys::ChannelInfo {
                channel: channel_id.index,
                is_input: channel_id.input.into(),
                ..unsafe { mem::zeroed() }
            };
        let code = unsafe { self.0.get_channel_info(&raw mut info) };
        create_result(info.into(), code)
    }

    /// # Safety
    /// * `callbacks` must outlive the created buffers.
    /// * Derefs of the returned buffer pointers must not.
    /// # Remarks
    /// Providing safe abstractions for this function is very difficult to do without getting highly opinionated,
    /// so it will remain `unsafe` for the time being. (Help wanted!)
	pub unsafe fn create_buffers(
        &self,
        channels: impl IntoIterator<Item=dto::ChannelId>,
        buffer_size: c_long,
        callbacks: *const sys::Callbacks
    )
    -> Result<impl Iterator<Item=[*mut c_void; 2]>>
    {
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
        
        let code = unsafe { self.0.create_buffers(infos.as_mut_ptr(), infos.len() as _, buffer_size, callbacks.cast_mut()) };
        let buffers =
            infos
            .into_iter()
            .map(|info| info.buffers);

        create_result(buffers, code)
    }

	pub fn dispose_all_buffers(&self) -> Result<()> {
        let code = unsafe { self.0.dispose_buffers() };
        create_result((), code)
    }

    /// Tells the driver to open its GUI
    pub fn open_control_panel(&self) -> Result<()> {
        let code = unsafe { self.0.control_panel() };
        create_result((), code)
    }

    /// A very unfortunate name. 
    /// This function actually has nothing to do with async code,
    /// it merely provides a mechanism for extending ASIO in the future.
    pub fn future<T: Future>(&self, param: &mut T::Param) -> Result<()> {
        let selector = T::SELECTOR.0;
        let opt = ptr::from_mut(param).cast();
        
        let code = unsafe { self.0.future(selector, opt) };
        create_result((), code)
    }
	
    /// Tells the driver that the host is done processing output buffers.
    /// 
    /// This is *not* implicitly inferred from the return of [`Callbacks::buffer_switch`] / [`Callbacks::buffer_switch_time_info`],
    /// because it might have been called by a thread that doesn't allow processing within the callback.
    /// 
    /// # Caveats
    /// Devices without hardware DSP and no further internal buffering
	/// have no use for this signal, so their drivers might not support it,
    /// and instead return [`ErrorCode::NOT_PRESENT`].
    /// This is not fatal, it just means that calls to this function can (and should) be skipped.
    /// Take care not to "error out" unnecessarily in this case.
    pub fn output_ready(&self) -> Result<()> {
        let code = unsafe { self.0.output_ready() };
        create_result((), code)
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Error(NonZeroI32);

impl Error {
    /// guaranteed to never be [`ErrorCode::OK`] / [`ErrorCode::SUCCESS`]
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        ErrorCode(self.0.get())
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