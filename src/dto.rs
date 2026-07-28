use std::ffi::{CString, c_long};
use crate::utils::cstring_from_bytes_until_nul;
use azo_sys as sys;

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
    pub position: sys::Samples,
    pub time_stamp: sys::TimeStamp
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelInfoResponse {
    pub is_active  : bool,
	pub group      : sys::ChannelGroup,
	pub sample_type: sys::SampleType,
	pub name       : CString
}

impl From<sys::ChannelInfo> for ChannelInfoResponse {
    fn from(value: sys::ChannelInfo) -> Self {
        Self {
            is_active  : value.is_active.try_into().unwrap_or(false),
            group      : value.channel_group,
            sample_type: value.sample_type,
            name       : cstring_from_bytes_until_nul(&value.name)
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
pub struct ChannelId {
    pub input: bool,
    pub index: sys::ChannelIndex
}