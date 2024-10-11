#![allow(clippy::pub_underscore_fields, reason = "placeholders")]
#![expect(clippy::transmute_ptr_to_ptr, reason = "occurs in a proc macro (`interface`)")]
use std::ffi::*;
use bitflags::bitflags;
use crate::windows_bindings::HWND;
use windows_core::{interface, IUnknown, IUnknown_Vtbl};
use crate::convert_cstring;

#[interface]
pub unsafe trait IDriver: IUnknown {
	pub fn init               (&self, sys_ref: HWND                                                                                      ) -> Bool;
	pub fn get_driver_name    (&self, name: *mut u8                                                                                      ) -> ();
	pub fn get_driver_version (&self,                                                                                                    ) -> DriverVersion;
	pub fn get_error_message  (&self, string: *mut u8                                                                                    ) -> ();
	pub fn start              (&self,                                                                                                    ) -> ErrorCode;
	pub fn stop               (&self,                                                                                                    ) -> ErrorCode;
	pub fn get_channels       (&self, num_input_channels: *mut c_long, num_output_channels: *mut c_long                                  ) -> ErrorCode;
	pub fn get_latencies      (&self, input_latency: *mut c_long, output_latency: *mut c_long                                            ) -> ErrorCode;
	pub fn get_buffer_size    (&self, min_size: *mut c_long, max_size: *mut c_long, preferred_size: *mut c_long, granularity: *mut c_long) -> ErrorCode;
	pub fn can_sample_rate    (&self, sample_rate: SampleRate                                                                            ) -> ErrorCode;
	pub fn get_sample_rate    (&self, sample_rate: *mut SampleRate                                                                       ) -> ErrorCode;
	pub fn set_sample_rate    (&self, sample_rate: SampleRate                                                                            ) -> ErrorCode;
	pub fn get_clock_sources  (&self, clocks: *mut ClockSource, num_sources: *mut c_long                                                 ) -> ErrorCode;
	pub fn set_clock_source   (&self, reference: ClockSourceIndex                                                                        ) -> ErrorCode;
	pub fn get_sample_position(&self, s_pos: *mut Samples, t_stamp: *mut TimeStamp                                                       ) -> ErrorCode;
	pub fn get_channel_info   (&self, info: *mut ChannelInfo                                                                             ) -> ErrorCode;
	pub fn create_buffers     (&self, buffer_infos: *mut BufferInfo, num_channels: c_long, buffer_size: c_long, callbacks: *mut Callbacks) -> ErrorCode;
	pub fn dispose_buffers    (&self,                                                                                                    ) -> ErrorCode;
	pub fn control_panel      (&self,                                                                                                    ) -> ErrorCode;
	pub fn future             (&self, selector: c_long, opt: *mut c_void                                                                 ) -> ErrorCode;
	pub fn output_ready       (&self,                                                                                                    ) -> ErrorCode;
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DriverVersion(pub c_long);

#[repr(transparent)]
#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, Hash)]
pub struct ChannelGroup(pub c_long);

#[repr(transparent)]
#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, Hash)]
pub struct ChannelIndex(pub c_long);

#[repr(transparent)]
#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, Hash)]
pub struct ClockSourceIndex(pub c_long);

pub type U31 = c_long; // todo

pub type Samples    = i64;
pub type TimeStamp  = i64;
pub type SampleRate = f64;

#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bool(pub c_long);
impl Bool {
	pub const FALSE: Self = Self(0);
	pub const TRUE : Self = Self(1);
}

impl From<bool> for Bool {
    fn from(value: bool) -> Self {
        Self(value as _)
    }
}

impl TryFrom<Bool> for bool {
	type Error = UndefinedValueError;
	
	fn try_from(value: Bool) -> Result<Self, Self::Error> {
		match value {
			Bool::TRUE => Ok(true),
			Bool::FALSE => Ok(false),
			_ => Err(UndefinedValueError)
		}
	}
}

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct UndefinedValueError;

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SampleType(pub c_long);
impl SampleType {
	pub const PCM_I16_MSB: Self = Self(0);
	pub const PCM_I24_MSB: Self = Self(1);
	pub const PCM_I32_MSB: Self = Self(2);
	pub const PCM_F32_MSB: Self = Self(3);
	pub const PCM_F64_MSB: Self = Self(4);
	// 5
	// 6
	// 7
	pub const PCM_I32_MSB_16: Self = Self(8);
	pub const PCM_I32_MSB_18: Self = Self(9);
	pub const PCM_I32_MSB_20: Self = Self(10);
	pub const PCM_I32_MSB_24: Self = Self(11);
	// 12
	// 13
	// 14
	// 15
	pub const PCM_I16_LSB: Self = Self(16);
	pub const PCM_I24_LSB: Self = Self(17);
	pub const PCM_I32_LSB: Self = Self(18);
	pub const PCM_F32_LSB: Self = Self(19);
	pub const PCM_F64_LSB: Self = Self(20);
	// 21
	// 22
	// 23
	pub const PCM_I32_LSB_16: Self = Self(24);
	pub const PCM_I32_LSB_18: Self = Self(25);
	pub const PCM_I32_LSB_20: Self = Self(26);
	pub const PCM_I32_LSB_24: Self = Self(27);
	// 28
	// 29
	// 30
	// 31
	pub const DSD_I8_LSB_1: Self = Self(32);
	pub const DSD_I8_MSB_1: Self = Self(33);
	pub const DSD_I8_NER_8: Self = Self(40);
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ErrorCode(pub c_long);
impl ErrorCode {
    pub const OK               : Self = Self(0);
    pub const SUCCESS          : Self = Self(0x3f4847a0);
    pub const NOT_PRESENT      : Self = Self(-1000);
    pub const HW_MALFUNCTION   : Self = Self(-999);
    pub const INVALID_PARAMETER: Self = Self(-998);
    pub const INVALID_MODE     : Self = Self(-997);
    pub const SP_NOT_ADVANCING : Self = Self(-996);
    pub const NO_CLOCK         : Self = Self(-995);
    pub const NO_MEMORY        : Self = Self(-994);
	
	pub fn ok<T>(self, ok_value: T) -> Result<T, Self>{
        match self {
            Self::OK |
            Self::SUCCESS => Ok(ok_value),
            
			error => Err(error)
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
pub struct TimeCode {
	pub speed: c_double,
	pub time_code_samples: Samples,
	pub flags: TimeCodeFlags,
	pub _placeholder: [c_char; 64]
}

bitflags! {
	#[repr(transparent)]
	#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
	pub struct TimeCodeFlags: c_ulong {
		const VALID       = 1 << 0;
		const RUNNING     = 1 << 1;
		const REVERSE     = 1 << 2;
		const ONSPEED     = 1 << 3;
		const STILL       = 1 << 4;
		// const ???      = 1 << 5;
		// const ???      = 1 << 6;
		// const ???      = 1 << 7;
		const SPEED_VALID = 1 << 8;
	}
}

#[repr(C)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TimeInfo {
	pub speed          : c_double,
	pub system_time    : TimeStamp,
	pub sample_position: Samples,
	pub sample_rate    : SampleRate,
	pub flags          : TimeInfoFlags,
	pub reserved       : [c_char; 12]
}

bitflags! {
	#[repr(transparent)]
	#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
	pub struct TimeInfoFlags: c_ulong {
		const SYSTEM_TIME_VALID     = 1 << 0;
		const SAMPLE_POSITION_VALID = 1 << 1;
		const SAMPLE_RATE_VALID     = 1 << 2;
		const SPEED_VALID           = 1 << 3;
		const SAMPLE_RATE_CHANGED   = 1 << 4;
		const CLOCK_SOURCE_CHANGED  = 1 << 5;
	}
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
pub struct Time {
    pub _reserved: [c_long; 4],
    pub time_info: TimeInfo,
    pub time_code: TimeCode,
}


#[repr(C)]
#[derive(Debug, Clone, Hash)]
pub struct Callbacks {
	/// * `double_buffer_index` points to the half that host should read/write.
	/// * `direct_process` indicates whether or not it is safe to do processing on the calling thread.
    pub buffer_switch:
		unsafe extern "system" fn(
			double_buffer_index: c_long, 
			direct_process     : Bool
		),
    
    /// 0 = unknown (e.g. in case of clock loss)
    pub sample_rate_did_change:
		unsafe extern "system" fn(
			sample_rate: SampleRate
		),

	/// See the constants on [`MessageSelector`] for info on params and returns.
    pub asio_message:
		unsafe extern "system" fn(
			selector: MessageSelector,
			value   : c_long,
			message : *const c_void,
			opt     : *const f64
		) -> c_long,

	/// Similar to [`Self::buffer_switch`], but with additional timing info.
    pub buffer_switch_time_info:
		unsafe extern "system" fn(
			params             : *const Time,
			double_buffer_index: c_long,
			direct_process     : Bool
		) -> Time
}

/// Used for driver-to-host messages via [`Callbacks::asio_message`]
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageSelector(pub c_long);

impl MessageSelector {
	/// * Host returns [`Bool`] indicating whether the [`MessageSelector`] specified in `value` is supported.
	pub const SELECTOR_SUPPORTED: Self = Self(1);

    /// * Host returns its ASIO version (2+)
    pub const ENGINE_VERSION: Self = Self(2);

	/// The host should release the COM interface and start over.
	/// * Host returns [`Bool`] indicating whether the request will be honored
	pub const RESET_REQUEST: Self = Self(3);
    
	/// The driver resizes its buffers to `value`.
	/// * Host returns [`Bool`] indicating compatibility
	pub const BUFFER_SIZE_CHANGE: Self = Self(4);
    
	/// The driver's timings desynced.
    /// * Host returns [`Bool`] indicating resync support
	pub const RESYNC_REQUEST: Self = Self(5);
    
    /// The host needs to re-fetch the latencies.
	/// * Host returns [`Bool`] indicating whether this selector is supported
	pub const LATENCIES_CHANGED: Self = Self(6);	
    
	/// Whether the host supports [`Callbacks::buffer_switch_time_info`]
    /// * Host returns [`Bool`] indicating support
	pub const SUPPORTS_TIME_INFO: Self = Self(7);
    
	/// Whether the host supports [`Time::time_code`] in [`Callbacks::buffer_switch_time_info`]
	/// * Host returns [`Bool`] indicating support
	pub const SUPPORTS_TIME_CODE: Self = Self(8);
    
	/// <div class="warning">OMITTED FROM SPEC</div>
	/// 
    /// * `value` indicates command count
	/// * `message` provides the commands
	#[cfg(feature = "spec_omitted")]
	pub const MMC_COMMAND: Self = Self(9);

	/// <div class="warning">OMITTED FROM SPEC</div>
	/// 
	/// * Host returns [`Bool`]
	#[cfg(feature = "spec_omitted")]
	pub const SUPPORTS_INPUT_MONITOR: Self = Self(10);
	
	/// <div class="warning">OMITTED FROM SPEC</div>
	/// 
	/// * Host returns [`Bool`]
	#[cfg(feature = "spec_omitted")]
	pub const SUPPORTS_INPUT_GAIN: Self = Self(11);
	
	/// <div class="warning">OMITTED FROM SPEC</div>
	/// 
	/// * Host returns [`Bool`]
	#[cfg(feature = "spec_omitted")]
	pub const SUPPORTS_INPUT_METER: Self = Self(12);
	
	/// <div class="warning">OMITTED FROM SPEC</div>
	/// 
	/// * Host returns [`Bool`]
	#[cfg(feature = "spec_omitted")]
	pub const SUPPORTS_OUTPUT_GAIN: Self = Self(13);
	
	/// <div class="warning">OMITTED FROM SPEC</div>
	/// 
	/// * Host returns [`Bool`]
	#[cfg(feature = "spec_omitted")]
	pub const SUPPORTS_OUTPUT_METER: Self = Self(14);
	
	/// The driver detected an overload
	/// * Host returns whatever it wants (driver may ignore it)
	pub const OVERLOAD: Self = Self(15);
}

#[repr(C)]
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct ClockSource {
    /// For use in [`IDriver::set_clock_source()`]
    pub index: ClockSourceIndex,
    
	/// E.g. S/PDIF, AES/EBU
	pub associated_channel: ChannelIndex,

	pub associated_group: ChannelGroup,
	
	pub is_current_source: Bool,
	
	pub name: [u8; 32]
}

impl ClockSource {
	#[must_use]
	pub fn name(&self) -> String {
		convert_cstring(&self.name)
	}
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelInfo {
	pub channel      : ChannelIndex,
	pub is_input     : Bool,
	pub is_active    : c_long,
	pub channel_group: ChannelGroup,
	pub sample_type  : SampleType,
	pub name         : [u8; 32]
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BufferInfo {
	pub is_input   : Bool,
	pub channel_num: ChannelIndex,
	pub buffers    : [*mut c_void; 2]
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FutureSelector(pub c_long);

#[expect(clippy::unusual_byte_groupings, reason = "easter eggs")]
impl FutureSelector {
	pub const ENABLE_TIME_CODE_READ : Self = Self(1);
	pub const DISABLE_TIME_CODE_READ: Self = Self(2);
	pub const SET_INPUT_MONITOR     : Self = Self(3);
	
	pub const SET_IO_FORMAT   : Self = Self(0x_23_11_1961);
	pub const GET_IO_FORMAT   : Self = Self(0x_23_11_1983);
	pub const CAN_DO_IO_FORMAT: Self = Self(0x_23_11_2004);
	
	pub const CAN_REPORT_OVERLOAD        : Self = Self(0x_24_04_2012);
	pub const GET_INTERNAL_BUFFER_SAMPLES: Self = Self(0x_25_04_2012);
}

#[cfg(feature = "spec_omitted")]
impl FutureSelector {
	pub const TRANSPORT        : Self = Self( 4);
	pub const SET_INPUT_GAIN   : Self = Self( 5);
	pub const GET_INPUT_METER  : Self = Self( 6);
	pub const SET_OUTPUT_GAIN  : Self = Self( 7);
	pub const GET_OUTPUT_METER : Self = Self( 8);
	pub const CAN_INPUT_MONITOR: Self = Self( 9);
	pub const CAN_TIME_INFO    : Self = Self(10);
	pub const CAN_TIME_CODE    : Self = Self(11);
	pub const CAN_TRANSPORT    : Self = Self(12);
	pub const CAN_INPUT_GAIN   : Self = Self(13);
	pub const CAN_INPUT_METER  : Self = Self(14);
	pub const CAN_OUTPUT_GAIN  : Self = Self(15);
	pub const CAN_OUTPUT_METER : Self = Self(16);
	pub const OPTIONAL_ONE     : Self = Self(17);
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InputMonitor {
	pub input: c_long,

	pub output: c_long,
	
	/// `0` = -inf dB<br>
	/// [`i32::MAX`] = +12 dB
	pub gain: U31,

	pub state: Bool,
	
	/// `0` = max left<br>
	/// [`i32::MAX`] = max right
	pub pan: U31
}

#[cfg(feature = "spec_omitted")]
#[repr(C)]
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct ChannelControls {
	/// in-param
	pub channel: c_long,
	
	/// in-param
	pub is_input: Bool,

	/// out-param
	pub gain: U31,

	/// out-param
	pub meter: U31,

	pub _placeholder: [c_char; 32]
}

#[cfg(feature = "spec_omitted")]
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransportParameters {
	pub command        : TransportParametersCommand,
	pub sample_position: Samples,
	pub track          : c_long,
	pub track_switches : [c_long; 16],
	pub _placeholder   : [c_char; 64]
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransportParametersCommand(pub c_long);

impl TransportParametersCommand {
	pub const START      : Self = Self( 1);
	pub const STOP       : Self = Self( 2);
	pub const LOCATE     : Self = Self( 3);
	pub const PUNCH_IN   : Self = Self( 4);
	pub const PUNCH_OUT  : Self = Self( 5);
	pub const ARM_ON     : Self = Self( 6);
	pub const ARM_OFF    : Self = Self( 7);
	pub const MONITOR_ON : Self = Self( 8);
	pub const MONITOR_OFF: Self = Self( 9);
	pub const ARM        : Self = Self(10);
	pub const MONITOR    : Self = Self(11);
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IoFormatType(pub c_long);
impl IoFormatType {
	pub const INVALID: Self = Self(-1);
	pub const PCM    : Self = Self( 0);
	pub const DSD    : Self = Self( 1);
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IoFormat {
	pub format_type: IoFormatType,
	pub _placeholder: [c_char; 512 - size_of::<IoFormatType>()]
}

#[repr(C)]
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct InternalBufferInfo {
	pub input_samples: c_long,
	pub output_samples: c_long
}