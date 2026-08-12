use std::ffi::CStr;
use azo::dto::{ChannelId, Latencies};
use azo::{Driver, sys::*};
use azo::future::*;

fn main() {
	let driver_metas = azo::get_drivers().unwrap();
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
	if !driver.init(None) {
		println!("init failed - {:?}", driver.last_error());
	}

	let driver_name    = driver.name();
	let driver_version = driver.version();
	let channel_counts = driver.channel_counts()?;
	let latencies      = driver.latencies().unwrap_or(Latencies { in_: -1, out: -1 }); // not all drivers support this as it is not strictly necessary for basic usage
	let buffer_size    = driver.buffer_size()?;
	let sample_rate    = driver.get_sample_rate()?;
	
	println!("driver_name   : {}", driver_name.to_string_lossy());
	println!("driver_version: {driver_version}");
	println!("channels      : {} in, {} out", channel_counts.in_, channel_counts.out);
	println!("latencies     : {} in, {} out", latencies.in_, latencies.out);
	println!("sample_rate   : {sample_rate}");
	print!  ("buffer_size   : {}", buffer_size.min);
	if buffer_size.max == buffer_size.min {
 		println!(" (fixed size)");
 	} else {
 		println!("..={} (pref {})", buffer_size.max, buffer_size.preferred);
		if let Some(granularity) = buffer_size.granularity {
			println!("                {granularity:?}");
		}
 	}
	println!();
	
	let mut io_format_pcm = IoFormat {
		format_type: IoFormatType::PCM,
		_placeholder: [0; _]
	};
	let mut io_format_dsd = IoFormat {
		format_type: IoFormatType::DSD,
		_placeholder: [0; _]
	};
	
	{
		println!("future selectors:");
		println!("\t{:<18}: {}" , stringify!(EnableTimeCodeRead), driver.future::<EnableTimeCodeRead>(&mut ()           ).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
		println!("\t{:<18}: {}" , stringify!(CanInputMonitor   ), driver.future::<CanInputMonitor   >(&mut ()           ).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
		println!("\t{:<18}: {}" , stringify!(CanTimeInfo       ), driver.future::<CanTimeInfo       >(&mut ()           ).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
		println!("\t{:<18}: {}" , stringify!(CanTimeCode       ), driver.future::<CanTimeCode       >(&mut ()           ).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
		println!("\t{:<18}: {}" , stringify!(CanReportOverload ), driver.future::<CanReportOverload >(&mut ()           ).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
		println!("\t{} PCM : {}", stringify!(CanDoIoFormat     ), driver.future::<CanDoIoFormat     >(&mut io_format_pcm).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
		println!("\t{} DSD : {}", stringify!(CanDoIoFormat     ), driver.future::<CanDoIoFormat     >(&mut io_format_dsd).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
	}

	#[cfg(feature = "undocumented")]
	{
		println!("undocumented future selectors:");
		println!("\t{:<18}: {}" , stringify!(CanTransport      ), driver.future::<CanTransport      >(&mut ()           ).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
		println!("\t{:<18}: {}" , stringify!(CanInputGain      ), driver.future::<CanInputGain      >(&mut ()           ).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
		println!("\t{:<18}: {}" , stringify!(CanInputMeter     ), driver.future::<CanInputMeter     >(&mut ()           ).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
		println!("\t{:<18}: {}" , stringify!(CanOutputGain     ), driver.future::<CanOutputGain     >(&mut ()           ).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
		println!("\t{:<18}: {}" , stringify!(CanOutputMeter    ), driver.future::<CanOutputMeter    >(&mut ()           ).map_or_else(|error| error.code(), |()| ResultCode::SUCCESS));
	}
	println!();
	
	println!("channel\tactive\tgroup\tsmpl_ty\tname");
	println!("...................................................");
	
	let fn_print_channel_info =
		|input, index| -> azo::Result<()> {
			let channel_info = driver.channel_info(ChannelId { input, index })?;
			println!(
				"{} {}\t{}\t{}\t{}\t{:?}",
				if input { " in" } else { "out" },
				index,
				channel_info.is_active,
				channel_info.group,
				channel_info.sample_type.0,
				channel_info.name,
			);
			Ok(())
		};
	
	for i in 0..channel_counts.in_ {
		fn_print_channel_info(true, i)?;
	}
	
	for i in 0..channel_counts.out {
		fn_print_channel_info(false, i)?;
	}

	println!();

	println!("clock\tchannel\tgroup\tcurrent\tname");
	println!("...................................................");
	
	let clock_sources = driver.clock_sources()?;

	#[expect(clippy::unwrap_in_result, reason = "simplicity")]
	for clock_source in clock_sources {
		println!(
			"{}\t{}\t{}\t{}\t{:?}",
			clock_source.index,
			clock_source.associated_channel,
			clock_source.associated_group,
			bool::try_from(clock_source.is_current_source).expect("invalid value"),
			CStr::from_bytes_until_nul(&clock_source.name).expect("buffer overflow")
		);
	}
	
	Ok(())
}