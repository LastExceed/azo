use std::ffi::c_long;
use crate::ffi;
use crate::utils::convert_cstring;


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelCounts {
    pub in_: c_long,
    pub out: c_long
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Latencies {
    pub in_: c_long,
    pub out: c_long
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SamplePosition {
    pub position: ffi::Samples,
    pub time_stamp: ffi::TimeStamp
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelInfoResponse {
    pub is_active  : bool,
	pub group      : ffi::ChannelGroup,
	pub sample_type: ffi::SampleType,
	pub name       : String
}

impl From<ffi::ChannelInfo> for ChannelInfoResponse {
    fn from(value: ffi::ChannelInfo) -> Self {
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
    pub channel: ffi::ChannelIndex
}