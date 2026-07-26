use super::ffi::*;

/// <div class="warning">
/// 
/// Not to be confused with [`std::future::Future`] !
/// 
/// </div>
pub trait Future {
	const SELECTOR: FutureSelector;
	type Param;
}

macro_rules! Impl {
	($($name:ident, $selector:ident, $param:ty),+) => {
		$(
			#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
			pub struct $name;
			
			impl Future for $name {
				const SELECTOR: FutureSelector = FutureSelector::$selector;
				type Param = $param;
			}
		)+
	}
}

Impl!(
	EnableTimeCodeRead , ENABLE_TIME_CODE_READ , (),
	DisableTimeCodeRead, DISABLE_TIME_CODE_READ, (),
	SetInputMonitor    , SET_INPUT_MONITOR     , InputMonitor,
	
	CanInputMonitor, CAN_INPUT_MONITOR, (),
	CanTimeInfo    , CAN_TIME_INFO    , (),
	CanTimeCode    , CAN_TIME_CODE    , (),
	
	SetIoFormat  , SET_IO_FORMAT   , IoFormat,
	GetIoFormat  , GET_IO_FORMAT   , IoFormat,
	CanDoIoFormat, CAN_DO_IO_FORMAT, IoFormat,
	
	CanReportOverload       , CAN_REPORT_OVERLOAD        , (),
	GetInternalBufferSamples, GET_INTERNAL_BUFFER_SAMPLES, InternalBufferInfo
);

#[cfg(feature = "spec_omitted")]
Impl!(
	Transport      ,  TRANSPORT       , TransportParameters,
	SetInputGain   ,  SET_INPUT_GAIN  , ChannelControls,
	GetInputMeter  ,  GET_INPUT_METER , ChannelControls,
	SetOutputGain  ,  SET_OUTPUT_GAIN , ChannelControls,
	GetOutputMeter ,  GET_OUTPUT_METER, ChannelControls,

	CanTransport   , CAN_TRANSPORT   , (),
	CanInputGain   , CAN_INPUT_GAIN  , (),
	CanInputMeter  , CAN_INPUT_METER , (),
	CanOutputGain  , CAN_OUTPUT_GAIN , (),
	CanOutputMeter , CAN_OUTPUT_METER, (),
	OptionalOne    , OPTIONAL_ONE    , ()
);