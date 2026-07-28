use std::ffi::CStr;
use azo::dto::ChannelId;
use azo::{Driver, sys::*};
use azo::future::*;

fn main() {
	let driver_metas = azo::discover_drivers().unwrap();
	for (driver_meta_index, driver_meta) in driver_metas.into_iter().enumerate() {		
		println!("\n==================== driver #{driver_meta_index} ====================\n");
		println!("description: {}"  , driver_meta.description);
		println!("clsid & iid: {:?}", driver_meta.clsid);
		println!();
		
		let driver = driver_meta.create_instance().unwrap();

		if let Err(error) = dump_info(&driver) {
			println!("{error} - {:?}", driver.last_error());
		}
	}
}

fn dump_info(driver: &Driver) -> azo::Result<()> {
	driver.init(None)?;
		
	let driver_name    = driver.name();
	let driver_version = driver.version();
	let channel_counts = driver.channel_counts()?;
	let latencies      = driver.latencies()?;
	let buffer_size    = driver.buffer_size()?;
	let sample_rate    = driver.get_sample_rate()?;
	
	println!("driver_name   : {}"           , driver_name.to_string_lossy());
	println!("driver_version: {}"           , driver_version.0);
	println!("channels      : {} in, {} out", channel_counts.in_, channel_counts.out);
	println!("latencies     : {} in, {} out", latencies.in_, latencies.out);
	println!("buffer_size   : {} ({:?})"    , buffer_size.preferred, buffer_size.range);
	println!("sample_rate   : {sample_rate}");
	println!();
	
	let mut io_format_pcm = IoFormat {
		format_type: IoFormatType::PCM,
		_placeholder: [0; _]
	};
	let mut io_format_dsd = IoFormat {
		format_type: IoFormatType::DSD,
		_placeholder: [0; _]
	};
	
	println!("{:<18}: {}" , stringify!(EnableTimeCodeRead), driver.future::<EnableTimeCodeRead>(&mut ()           ).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{:<18}: {}" , stringify!(CanInputMonitor   ), driver.future::<CanInputMonitor   >(&mut ()           ).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{:<18}: {}" , stringify!(CanTimeInfo       ), driver.future::<CanTimeInfo       >(&mut ()           ).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{:<18}: {}" , stringify!(CanTimeCode       ), driver.future::<CanTimeCode       >(&mut ()           ).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{:<18}: {}" , stringify!(CanTransport      ), driver.future::<CanTransport      >(&mut ()           ).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{:<18}: {}" , stringify!(CanInputGain      ), driver.future::<CanInputGain      >(&mut ()           ).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{:<18}: {}" , stringify!(CanInputMeter     ), driver.future::<CanInputMeter     >(&mut ()           ).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{:<18}: {}" , stringify!(CanOutputGain     ), driver.future::<CanOutputGain     >(&mut ()           ).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{:<18}: {}" , stringify!(CanOutputMeter    ), driver.future::<CanOutputMeter    >(&mut ()           ).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{:<18}: {}" , stringify!(CanReportOverload ), driver.future::<CanReportOverload >(&mut ()           ).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{} PCM : {}", stringify!(CanDoIoFormat     ), driver.future::<CanDoIoFormat     >(&mut io_format_pcm).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!("{} DSD : {}", stringify!(CanDoIoFormat     ), driver.future::<CanDoIoFormat     >(&mut io_format_dsd).map_or_else(|error| error.code(), |()| ErrorCode::SUCCESS));
	println!();
	
	println!("channel\tactive\tgroup\tsmpl_ty\tname");
	println!("...................................................");
	
	for i in 0..channel_counts.in_ {
		let channel_info = driver.channel_info(ChannelId { input: true, index: i })?;
		println!(
			"out {}\t{}\t{}\t{}\t{:?}",
			i,
			bool::try_from(channel_info.is_active).unwrap(),
			channel_info.group.0,
			channel_info.sample_type.0,
			channel_info.name,
		);
	}
	
	for i in 0..channel_counts.out {
		let channel_info = driver.channel_info(ChannelId { input: false, index: i })?;
		println!(
			" in {}\t{}\t{}\t{}\t{:?}",
			i,
			bool::try_from(channel_info.is_active).unwrap(),
			channel_info.group.0,
			channel_info.sample_type.0,
			channel_info.name,
		);
	}

	println!();

	println!("clock\tchannel\tgroup\tcurrent\tname");
	println!("...................................................");
	
	let clock_sources = driver.clock_sources()?;
	for clock_source in clock_sources {
		println!(
			"{}\t{}\t{}\t{}\t{:?}",
			clock_source.index.0,
			clock_source.associated_channel,
			clock_source.associated_group.0,
			bool::try_from(clock_source.is_current_source).unwrap(),
			CStr::from_bytes_until_nul(&clock_source.name).expect("buffer overflow")
		);
	}
	
	Ok(())
}