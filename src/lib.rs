pub mod data;
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

use std::{fmt, mem, ptr};
use std::fmt::Display;
use std::ffi::*;
use windows_core::GUID;
use extend::ext;
use self::data::*;
use self::future::Future;
use self::utils::*;

pub use self::windows_bindings::{HWND, HANDLE, COINIT, COINIT_APARTMENTTHREADED};
pub use azo_sys as sys;
use sys::*;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverMetadata {
    pub clsid: GUID,
    pub description: String,
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
            .get_string("description")?;
        
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
    pub fn init(&self, main_window_handle: Option<HWND>) -> Result<()> {
        let sys_ref = main_window_handle.unwrap_or_default(); 

        let success =
            unsafe { self.0.init(sys_ref.0) }
            .try_into()
            .unwrap_or(false);
        
        if success {
            Ok(())
        } else {
            ErrorCode(-1) // no proper error code available here
            .to_result((), &self.0)
        }
    }
    
    #[must_use]
    pub fn name(&self) -> String {
        let mut buf = [0_u8; 32];
        unsafe {
            self.0.get_driver_name(buf.as_mut_ptr());
        }
        convert_cstring(&buf)
    }

    #[must_use]
    pub fn version(&self) -> DriverVersion {
        unsafe { self.0.get_driver_version() }
    }
    
    #[must_use]
    pub fn last_error(&self) -> String {
        let mut buf = [0_u8; 124];
        unsafe {
            self.0.get_error_message(buf.as_mut_ptr());
        }
        convert_cstring(&buf)
    }
    
    pub fn start(&self) -> Result<()> {
        unsafe { self.0.start() }
        .to_result((), &self.0)
    }
    
    pub fn stop(&self) -> Result<()> {
        unsafe { self.0.stop() }
        .to_result((), &self.0)
    }

	pub fn channel_counts(&self) -> Result<ChannelCounts> {
        let mut counts = ChannelCounts { in_: 0, out: 0 };
        
        unsafe { self.0.get_channels(&raw mut counts.in_, &raw mut counts.out) }
        .to_result(counts, &self.0)
    }

    pub fn latencies(&self) -> Result<Latencies> {
        let mut latencies = Latencies { in_: 0, out: 0 };
        
        unsafe { self.0.get_latencies(&raw mut latencies.in_, &raw mut latencies.out) }
        .to_result(latencies, &self.0)
    }

    pub fn buffer_size(&self) -> Result<BufferSize> {
        let mut out: BufferSize = unsafe { mem::zeroed() };
        
        unsafe { self.0.get_buffer_size(&raw mut out.min, &raw mut out.max, &raw mut out.preferred, &raw mut out.granularity) }
        .to_result(out, &self.0)
    }

	pub fn can_sample_rate(&self, sample_rate: SampleRate) -> Result<()> {
        unsafe { self.0.can_sample_rate(sample_rate) }
        .to_result((), &self.0)
    }
    
    pub fn get_sample_rate(&self) -> Result<SampleRate> {
        let mut sample_rate = f64::NAN;

        unsafe { self.0.get_sample_rate(&raw mut sample_rate) }
        .to_result(sample_rate, &self.0)
    }
    
    pub fn set_sample_rate(&self, sample_rate: SampleRate) -> Result<()> {
        unsafe { self.0.set_sample_rate(sample_rate) }
        .to_result((), &self.0)
    }
    
	pub fn clock_sources(&self) -> Result<Vec<ClockSource>> {
        let mut count = 1;
        let mut first = unsafe { mem::zeroed() };
        
        unsafe { self.0.get_clock_sources(&raw mut first, &raw mut count) }
        .to_result((), &self.0)?;
    
        if count < 1 {
            return Ok(vec![]);
        }
    
        if count == 1 {
            return Ok(vec![first]);
        }
        
        let mut all = vec![unsafe { mem::zeroed() }; count as _];

        unsafe { self.0.get_clock_sources(all.as_mut_ptr(), &raw mut count) }
        .to_result((), &self.0)?;
        
        Ok(all)
    }

	pub fn set_clock_source(&self, clock_source: ClockSourceIndex) -> Result<()> {
        unsafe { self.0.set_clock_source(clock_source) }
        .to_result((), &self.0)
    }

	pub fn sample_position(&self) -> Result<SamplePosition> {
        let mut sample_pos = SamplePosition { position: 0, time_stamp: 0 };
        
        unsafe { self.0.get_sample_position(&raw mut sample_pos.position, &raw mut sample_pos.time_stamp) }
        .to_result(sample_pos, &self.0)
    }

	pub fn channel_info(&self, channel_id: ChannelId) -> Result<ChannelInfoResponse> {
        let mut info =
            ChannelInfo {
                channel: channel_id.index,
                is_input: channel_id.input.into(),
                ..unsafe { mem::zeroed() }
            };

        unsafe { self.0.get_channel_info(&raw mut info) }
        .to_result(info.into(), &self.0)
    }

    /// # Safety
    /// * `callbacks` must outlive the created buffers.
    /// * Derefs of the returned buffer pointers must not.
    /// # Remarks
    /// Providing safe abstractions for this function is very difficult to do without getting highly opinionated,
    /// so it will remain `unsafe` for the time being. (Help wanted!)
	pub unsafe fn create_buffers(
        &self,
        channels: impl IntoIterator<Item=ChannelId>,
        buffer_size: c_long,
        callbacks: *const Callbacks
    )
    -> Result<Vec<[*mut c_void; 2]>>
    {
        let mut infos =
            channels
            .into_iter()
            .map(|ChannelId { input, index }|
                BufferInfo {
                    is_input: input.into(),
                    channel_num: index,
                    buffers: [ptr::null_mut(); 2]
                }
            )
            .collect::<Vec<_>>();
        
        unsafe { self.0.create_buffers(infos.as_mut_ptr(), infos.len() as _, buffer_size, callbacks.cast_mut()) }
        .to_result((), &self.0)?;
    
        let buffers =
            infos
            .into_iter()
            .map(|info| info.buffers)
            .collect();

        Ok(buffers)
    }

	pub fn dispose_all_buffers(&self) -> Result<()> {
        unsafe { self.0.dispose_buffers() }
        .to_result((), &self.0)
    }

    /// Tells the driver to open its GUI
    pub fn open_control_panel(&self) -> Result<()> {
        unsafe { self.0.control_panel() }
        .to_result((), &self.0)
    }

    /// A very unfortunate name. 
    /// This function actually has nothing to do with async code,
    /// it merely provides a mechanism for extending ASIO in the future.
    pub fn future<T: Future>(&self, param: &mut T::Param) -> Result<()> {
        let selector = T::SELECTOR.0;
        let opt = ptr::from_mut(param).cast();
        
        unsafe { self.0.future(selector, opt) }
        .to_result((), &self.0)
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
        unsafe { self.0.output_ready() }
        .to_result((), &self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String
}
#[expect(clippy::absolute_paths, reason = "name collision")]
impl std::error::Error for Error {}
impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code.0, self.message)
    }
}

#[expect(clippy::absolute_paths, reason = "name collision")]
pub type Result<T> = std::result::Result<T, Error>;

#[ext]
pub impl ErrorCode {
    fn to_result<T>(self, ok_value: T, interface: &IIASIORedecl) -> Result<T> {
        if matches!(self, Self::OK | Self::SUCCESS) {
            return Ok(ok_value);
        }
        
        let mut buf = [0_u8; 124];
        unsafe {
            interface.get_error_message(buf.as_mut_ptr());
        }
        
        let message = convert_cstring(&buf);
        
        Err(Error { code: self, message })
    }
}

#[ext]
pub impl ClockSource {
	fn name(&self) -> String {
		convert_cstring(&self.name)
	}
}