pub mod ffi;
pub mod future;
pub mod utils;
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
use windows_core::{GUID, IUnknown};
use self::future::Future;
use self::ffi::*;
use self::utils::*;
use self::windows_bindings::*;

pub use self::windows_bindings::{HWND, HANDLE};

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
        Driver::new(&self.clsid)
    }
}

#[derive(Debug)]
pub struct Driver(IDriver, COM);
impl Driver {
    fn new(guid: &GUID) -> WinResult<Self> {
        let com = COM::new(COINIT_APARTMENTTHREADED)?;
        let i_driver = unsafe { IDriver::create_instance(guid) }?;

        Ok(Self(i_driver, com))
    }
    
    pub fn init(&self, main_window_handle: Option<HWND>) -> Result<()> {
        let sys_ref = main_window_handle.unwrap_or_default(); 

        let success =
            unsafe { self.0.init(sys_ref) }
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
            self.0.get_driver_name(&raw mut buf[0]);
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
            self.0.get_error_message(&raw mut buf[0]);
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

	pub fn channel_count(&self) -> Result<(i32, i32)> {
        let mut n_in = 0;
        let mut n_out = 0;
        
        unsafe { self.0.get_channels(&raw mut n_in, &raw mut n_out) }
        .to_result((n_in, n_out), &self.0)
    }

    pub fn latencies(&self) -> Result<(i32, i32)> {
        let mut n_in = 0;
        let mut n_out = 0;
        
        unsafe { self.0.get_latencies(&raw mut n_in, &raw mut n_out) }
        .to_result((n_in, n_out), &self.0)
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

        unsafe { self.0.get_clock_sources(&raw mut all[0], &raw mut count) }
        .to_result((), &self.0)?;
        
        Ok(all)
    }

	pub fn set_clock_source(&self, clock_source: ClockSourceIndex) -> Result<()> {
        unsafe { self.0.set_clock_source(clock_source) }
        .to_result((), &self.0)
    }

	pub fn sample_position(&self) -> Result<(Samples, TimeStamp)> {
        let mut sample_position = 0;
        let mut time_stamp      = 0;
        
        unsafe { self.0.get_sample_position(&raw mut sample_position, &raw mut time_stamp) }
        .to_result((sample_position, time_stamp), &self.0)
    }

	pub fn channel_info(&self, channel: ChannelIndex, input: bool) -> Result<ChannelInfoResponse> {
        let mut info =
            ChannelInfo {
                channel,
                is_input: input.into(),
                ..unsafe { mem::zeroed() }
            };

        unsafe { self.0.get_channel_info(&raw mut info) }
        .to_result(info.into(), &self.0)
    }

	pub fn create_buffers<const COUNT: usize>(
        &self,
        args: &[BufferCreateArgs],
        buffer_size: c_long,
        callbacks: &mut Callbacks
    )
    -> Result<Vec<[*mut c_void; 2]>> 
    {
        let mut infos =
            args
            .iter()
            .map(|BufferCreateArgs { input, channel }|
                BufferInfo {
                    is_input: (*input).into(),
                    channel_num: *channel,
                    buffers: [ptr::null_mut(); 2]
                }
            )
            .collect::<Vec<_>>();
        
        unsafe { self.0.create_buffers(&raw mut infos[0], COUNT as _, buffer_size, callbacks) }
        .to_result((), &self.0)?;
    
        Ok(infos.into_iter().map(|info| info.buffers).collect())
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
    /// This is not implicitly inferred from the return of [`Callbacks::buffer_switch`] / [`Callbacks::buffer_switch_time_info`]
    /// because it might have been called by a thread that doesn't allow processing within the callback
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelInfoResponse {
    pub is_active  : bool,
	pub group      : ChannelGroup,
	pub sample_type: SampleType,
	pub name       : String
}

impl From<ChannelInfo> for ChannelInfoResponse {
    fn from(value: ChannelInfo) -> Self {
        Self {
            is_active  : value.is_active.try_into().unwrap_or(false),
            group      : value.channel_group,
            sample_type: value.sample_type,
            name       : convert_cstring(&value.name)
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct BufferSize {
    pub min: c_long,
    pub max: c_long,
    pub preferred: c_long,
    pub granularity: c_long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferCreateArgs {
    pub input: bool,
    pub channel: ChannelIndex
}

impl IDriver {
    /// # Safety
    /// COM must be initialized.
    unsafe fn create_instance(guid: *const GUID) -> WinResult<Self> {
        // In theory, `CoCreateInstance` could instanciate the IDriver interface directly.
        // However, the windows-rs wrapper of this function acquires the IID from a trait-associated constant,
        // which is impossible to implement in this case.
        let i_unknown: IUnknown =
            unsafe { CoCreateInstance(guid, None, CLSCTX_SERVER) }?;

        // The same limitation also applies to `.cast()`.
        // Luckily, the underlying `.query()` is public,
        // which enables the following work-around:
        unsafe { cast_decoupled(&i_unknown, guid) }
    }
}

impl ErrorCode {
    fn to_result<T>(self, ok_value: T, i_driver: &IDriver) -> Result<T> {
        if matches!(self, Self::OK | Self::SUCCESS) {
            return Ok(ok_value);
        }
        
        let mut buf = [0_u8; 124];
        unsafe {
            i_driver.get_error_message((&raw mut buf).cast());
        }
        
        let message = convert_cstring(&buf);
        
        Err(Error { code: self, message })
    }
}