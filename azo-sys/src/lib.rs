#![allow(clippy::pub_underscore_fields, reason = "placeholders")]
#![expect(clippy::transmute_ptr_to_ptr, reason = "occurs in a proc macro (`interface`)")]
#![expect(clippy::unusual_byte_groupings, reason = "easter eggs")]

#[macro_use]
mod utils;

use std::{ffi::*, mem};
use bitflags::bitflags;
use windows_core::{interface, IUnknown, IUnknown_Vtbl};

use self::utils::I64Split;

/// Bizarrely, there is no IID assigned to the original `IASIO`, which means it is not actually a COM interface at all.
/// Instead, each driver declares and implements an individual replica interface,
/// re-using the CLSID of its implementation as the IID for the replica.
///
/// That, together with the complete absence of `HRESULT`s in all functions breaking any form of marshalling,
/// is a horrible abuse of the COM system, and completely defeats the point of using it in the first place.
/// 
/// Since each driver's re-declaration is distinct, and the naming up to the respective developers, there is no correct answer to what the name of [`Self`] should be,
/// so a descriptive name is used here. To be really pedantic, this technically would even need to be called `IIIASIORedeclRedecl`,
/// as it re-declares the already re-decleared interface AGAIN. But that name would be excessively noisy, and probably cause more confusion than clarity.
#[interface]
pub unsafe trait IIASIORedecl: IUnknown {
	pub fn init               (&self, sys_ref: *mut c_void                                                                               ) -> Bool;
	pub fn get_driver_name    (&self, name: *mut u8                                                                                      ) -> ();
	pub fn get_driver_version (&self,                                                                                                    ) -> DriverVersion;
	pub fn get_error_message  (&self, string: *mut u8                                                                                    ) -> ();
	pub fn start              (&self,                                                                                                    ) -> ResultCode;
	pub fn stop               (&self,                                                                                                    ) -> ResultCode;
	pub fn get_channels       (&self, num_input_channels: *mut c_long, num_output_channels: *mut c_long                                  ) -> ResultCode;
	pub fn get_latencies      (&self, input_latency: *mut c_long, output_latency: *mut c_long                                            ) -> ResultCode;
	pub fn get_buffer_size    (&self, min_size: *mut c_long, max_size: *mut c_long, preferred_size: *mut c_long, granularity: *mut c_long) -> ResultCode;
	pub fn can_sample_rate    (&self, sample_rate: SampleRate                                                                            ) -> ResultCode;
	pub fn get_sample_rate    (&self, sample_rate: *mut SampleRate                                                                       ) -> ResultCode;
	pub fn set_sample_rate    (&self, sample_rate: SampleRate                                                                            ) -> ResultCode;
	pub fn get_clock_sources  (&self, clocks: *mut ClockSource, num_sources: *mut c_long                                                 ) -> ResultCode;
	pub fn set_clock_source   (&self, reference: ClockSourceIndex                                                                        ) -> ResultCode;
	pub fn get_sample_position(&self, s_pos: *mut Samples, t_stamp: *mut TimeStamp                                                       ) -> ResultCode;
	pub fn get_channel_info   (&self, info: *mut ChannelInfo                                                                             ) -> ResultCode;
	pub fn create_buffers     (&self, buffer_infos: *mut BufferInfo, num_channels: c_long, buffer_size: c_long, callbacks: *mut Callbacks) -> ResultCode;
	pub fn dispose_buffers    (&self,                                                                                                    ) -> ResultCode;
	pub fn control_panel      (&self,                                                                                                    ) -> ResultCode;
	pub fn future             (&self, selector: FutureSelector, opt: *mut c_void                                                         ) -> ResultCode;
	pub fn output_ready       (&self,                                                                                                    ) -> ResultCode;
}

pub type DriverVersion     = c_long;
pub type ChannelGroupIndex = c_long;
pub type ChannelIndex      = c_long;
pub type ClockSourceIndex  = c_long;
pub type NonNegativeI32    = c_long;
pub type Samples    = I64Split;
pub type TimeStamp  = I64Split;
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UndefinedValueError;

c_enum!(SampleType,
	PCM_I16_MSB = 0,
	PCM_I24_MSB = 1,
	PCM_I32_MSB = 2,
	PCM_F32_MSB = 3,
	PCM_F64_MSB = 4,
	// 5, 6, 7
	PCM_I32_MSB_16 = 8,
	PCM_I32_MSB_18 = 9,
	PCM_I32_MSB_20 = 10,
	PCM_I32_MSB_24 = 11,
	// 12, 13, 14, 15
	PCM_I16_LSB = 16,
	PCM_I24_LSB = 17,
	PCM_I32_LSB = 18,
	PCM_F32_LSB = 19,
	PCM_F64_LSB = 20,
	// 21, 22, 23
	PCM_I32_LSB_16 = 24,
	PCM_I32_LSB_18 = 25,
	PCM_I32_LSB_20 = 26,
	PCM_I32_LSB_24 = 27,
	// 28, 29, 30, 31
	DSD_I8_LSB_1 = 32,
	DSD_I8_MSB_1 = 33,
	DSD_I8_NER_8 = 40
);

c_enum!(
	/// Formery "Error"
	ResultCode,

	OK                = 0,
	SUCCESS           = 0x3f4847a0,
	NOT_PRESENT       = -1000,
	HW_MALFUNCTION    = -999,
	INVALID_PARAMETER = -998,
	INVALID_MODE      = -997,
	SP_NOT_ADVANCING  = -996,
	NO_CLOCK          = -995,
	NO_MEMORY         = -994
);

impl ResultCode {
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

impl TimeCode {
	/// Creates an "invalid" instance of `Self`,
	/// meaning that the [`TimeCodeFlags::VALID`] bit in [`TimeCode::flags`] is not set,
	/// and the remaining values are unspecified.
	#[must_use]
	pub const fn invalid() -> Self {
		// SAFETY:
		// `Self` is valid for any bit pattern
		unsafe { mem::zeroed() }
	}
}

impl Default for TimeCode {
	fn default() -> Self {
		Self::invalid()
	}
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
	/// Must be zeroed
	pub _reserved      : [c_char; 12]
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
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Time {
    pub _reserved: [c_long; 4],
    pub time_info: TimeInfo,
    pub time_code: TimeCode,
}

/// * `double_buffer_index` points to the half that host should read/write.
/// * `direct_process` indicates whether or not it is safe to do processing on the calling thread.
pub type BufferSwitch =
	unsafe extern "system" fn(
		double_buffer_index: c_long, 
		direct_process     : Bool
	);
	
/// 0 = unknown (e.g. in case of clock loss)
pub type SampleRateDidChange =
	unsafe extern "system" fn(
		sample_rate: SampleRate
	);

/// See the constants on [`MessageSelector`] for info on params and returns.
pub type AsioMessage =
	unsafe extern "system" fn(
		selector: MessageSelector,
		value   : c_long,
		message : *const c_void,
		opt     : *const f64
	) -> c_long;
	
/// Similar to [`BufferSwitch`], but with additional timing info.
pub type BufferSwitchTimeInfo =
	unsafe extern "system" fn(
		params             : *mut Time,
		double_buffer_index: c_long,
		direct_process     : Bool
	) -> *mut Time;

#[repr(C)]
#[derive(Debug, Clone, Hash)]
pub struct Callbacks {
    pub buffer_switch          : BufferSwitch,
    pub sample_rate_did_change : SampleRateDidChange,
    pub asio_message           : AsioMessage,
    pub buffer_switch_time_info: BufferSwitchTimeInfo
}

impl Callbacks {
	/// Valid pointers to static empty functions
	pub const NOOP: Self = Self {
		buffer_switch          : noop_buffer_switch,
		sample_rate_did_change : noop_sample_rate_did_change,
		asio_message           : noop_asio_message,
		buffer_switch_time_info: noop_buffer_switch_time_info
	};
}

const unsafe extern "system" fn noop_buffer_switch(_: i32, _: Bool) {}
const unsafe extern "system" fn noop_sample_rate_did_change(_: f64) {}
const unsafe extern "system" fn noop_asio_message(_: MessageSelector, _: i32, _: *const c_void, _: *const f64) -> i32 { Bool::FALSE.0 }
const unsafe extern "system" fn noop_buffer_switch_time_info(time: *mut Time, _: i32, _: Bool) -> *mut Time { time }

c_enum!(
	/// Used for driver-to-host messages via [`Callbacks::asio_message`]
	MessageSelector,

	/// * Host returns [`Bool`] indicating whether the [`MessageSelector`] specified in `value` is supported.
	SELECTOR_SUPPORTED = 1,

    /// * Host returns its ASIO version (2+)
    ENGINE_VERSION = 2,

	/// The host should release the COM interface and start over.
	/// * Host returns [`Bool`] indicating whether the request will be honored
	RESET_REQUEST = 3,
    
	/// The driver resizes its buffers to `value`.
	/// * Host returns [`Bool`] indicating compatibility
	BUFFER_SIZE_CHANGE = 4,
    
	/// The driver's timings desynced.
    /// * Host returns [`Bool`] indicating resync support
	RESYNC_REQUEST = 5,
    
    /// The host needs to re-fetch the latencies.
	/// * Host returns [`Bool`] indicating whether this selector is supported
	LATENCIES_CHANGED = 6,	
    
	/// Whether the host supports [`Callbacks::buffer_switch_time_info`]
    /// * Host returns [`Bool`] indicating support
	SUPPORTS_TIME_INFO = 7,
    
	/// Whether the host supports [`Time::time_code`] in [`Callbacks::buffer_switch_time_info`]
	/// * Host returns [`Bool`] indicating support
	SUPPORTS_TIME_CODE = 8,
	
	/// The driver detected an overload
	/// * Host returns whatever it wants (driver may ignore it)
	OVERLOAD = 15
);


#[cfg(feature = "undocumented")]
impl MessageSelector {
    /// * `value` indicates command count
	/// * `message` provides the commands
	pub const MMC_COMMAND: Self = Self(9);

	/// * Host returns [`Bool`]
	pub const SUPPORTS_INPUT_MONITOR: Self = Self(10);
	
	/// * Host returns [`Bool`]
	pub const SUPPORTS_INPUT_GAIN: Self = Self(11);
	
	/// * Host returns [`Bool`]
	pub const SUPPORTS_INPUT_METER: Self = Self(12);
	
	/// * Host returns [`Bool`]
	pub const SUPPORTS_OUTPUT_GAIN: Self = Self(13);
	
	/// * Host returns [`Bool`]
	pub const SUPPORTS_OUTPUT_METER: Self = Self(14);
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClockSource {
    /// For use in [`IIASIORedecl::set_clock_source()`]
    pub index: ClockSourceIndex,
    
	/// E.g. S/PDIF, AES/EBU
	pub associated_channel: ChannelIndex,

	pub associated_group: ChannelGroupIndex,
	
	pub is_current_source: Bool,
	
	pub name: [u8; 32]
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelInfo {
	pub channel      : ChannelIndex,
	pub is_input     : Bool,
	pub is_active    : Bool,
	pub channel_group: ChannelGroupIndex,
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

c_enum!(
	/// Used for host-to-driver messages via [`IIASIORedecl::future`]
	FutureSelector,

	ENABLE_TIME_CODE_READ  = 1,
	DISABLE_TIME_CODE_READ = 2,
	SET_INPUT_MONITOR      = 3,

	CAN_INPUT_MONITOR =  9,
	CAN_TIME_INFO     = 10,
	CAN_TIME_CODE     = 11,

	// DSD
	SET_IO_FORMAT    = 0x_23_11_1961,
	GET_IO_FORMAT    = 0x_23_11_1983,
	CAN_DO_IO_FORMAT = 0x_23_11_2004,

	// Drop out detection
	CAN_REPORT_OVERLOAD         = 0x_24_04_2012,
	GET_INTERNAL_BUFFER_SAMPLES = 0x_25_04_2012
);

#[cfg(feature = "undocumented")]
impl FutureSelector {
	pub const TRANSPORT        : Self = Self( 4);
	pub const SET_INPUT_GAIN   : Self = Self( 5);
	pub const GET_INPUT_METER  : Self = Self( 6);
	pub const SET_OUTPUT_GAIN  : Self = Self( 7);
	pub const GET_OUTPUT_METER : Self = Self( 8);
	
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
	/// `-1` = all
	pub input: ChannelIndex,

	pub output: ChannelIndex,
	
	/// `0` = -inf dB<br>
	/// `0x20000000` = 0 dB<br>
	/// [`i32::MAX`] = +12 dB
	pub gain: NonNegativeI32,

	pub state: Bool,
	
	/// `0` = max left<br>
	/// [`i32::MAX`] = max right
	pub pan: NonNegativeI32
}

#[cfg(feature = "undocumented")]
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelControls {
	/// in-param
	pub channel: ChannelIndex,
	
	/// in-param
	pub is_input: Bool,

	/// out-param
	pub gain: NonNegativeI32,

	/// out-param
	pub meter: NonNegativeI32,

	pub _placeholder: [c_char; 32]
}

#[cfg(feature = "undocumented")]
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransportParameters {
	pub command        : TransportParametersCommand,
	pub sample_position: Samples,
	pub track          : c_long,
	pub track_switches : [c_long; 16],
	pub _placeholder   : [c_char; 64]
}

c_enum!(TransportParametersCommand,
	START       =  1,
	STOP        =  2,
	LOCATE      =  3,
	PUNCH_IN    =  4,
	PUNCH_OUT   =  5,
	ARM_ON      =  6,
	ARM_OFF     =  7,
	MONITOR_ON  =  8,
	MONITOR_OFF =  9,
	ARM         = 10,
	MONITOR     = 11
);

c_enum!(IoFormatType,
	INVALID = -1,
	PCM     =  0,
	DSD     =  1
);

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IoFormat {
	pub format_type: IoFormatType,
	pub _placeholder: [c_char; 512 - size_of::<IoFormatType>()]
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InternalBufferInfo {
	pub input_samples: c_long,
	pub output_samples: c_long
}