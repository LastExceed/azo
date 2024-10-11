#![expect(clippy::unusual_byte_groupings, reason = "easter eggs")]

use std::ffi::c_long;

use super::ffi::*;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selector(pub c_long);

/// <div class="warning">
/// 
/// Not to be confused with [`std::future::Future`] !
/// 
/// </div>
pub trait Future {
	const SELECTOR: Selector;
	type Param;
}

macro_rules! Impl {
	($($name:ident, $value:literal, $param:ty),+) => {
		$(
			#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
			pub struct $name;
			
			impl Future for $name {
				const SELECTOR: Selector = Selector($value);
				type Param = $param;
			}
		)+
	}
}

Impl!(
	EnableTimeCodeRead , 1, (),
	DisableTimeCodeRead, 2, (),
	SetInputMonitor    , 3, InputMonitor
);

#[cfg(feature = "spec_omitted")]
Impl!(
	Transport      ,  4, TransportParameters,
	SetInputGain   ,  5, ChannelControls,
	GetInputMeter  ,  6, ChannelControls,
	SetOutputGain  ,  7, ChannelControls,
	GetOutputMeter ,  8, ChannelControls,
	CanInputMonitor,  9, (),
	CanTimeInfo    , 10, (),
	CanTimeCode    , 11, (),
	CanTransport   , 12, (),
	CanInputGain   , 13, (),
	CanInputMeter  , 14, (),
	CanOutputGain  , 15, (),
	CanOutputMeter , 16, (),
	OptionalOne    , 17, ()
);

Impl!( // DSD
	SetIoFormat  , 0x_23_11_1961, IoFormat,
	GetIoFormat  , 0x_23_11_1983, IoFormat,
	CanDoIoFormat, 0x_23_11_2004, IoFormat
);

Impl!( // Drop out detection
	CanReportOverload       , 0x_24_04_2012, (),
	GetInternalBufferSamples, 0x_25_04_2012, InternalBufferInfo
);