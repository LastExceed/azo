use azo::ffi::*;
use azo::future::*;

fn main() -> Result<(), Box<dyn core::error::Error>> {
	let driver_metas = azo::discover_drivers()?;
	for (driver_meta_index, driver_meta) in driver_metas.into_iter().enumerate() {		
		println!("\n==================== driver #{driver_meta_index} ====================\n");
		println!("clsid      : {}", driver_meta.clsid);
		println!("description: {}", driver_meta.description);
		println!();
		
		let driver = driver_meta.create_instance()?;
		
		driver.init(None)?;
		
		let driver_name     = driver.name();
		let driver_version  = driver.version();
		let channels        = driver.channel_count()?;
		let latencies       = driver.latencies()?;
		let buffer_size     = driver.buffer_size()?;
		let get_sample_rate = driver.get_sample_rate()?;
		let sample_position = driver.sample_position();
		
		println!("driver_name     : {driver_name:?}"    );
		println!("driver_version  : {driver_version:?}" );
		println!("channels        : {channels:?}"       );
		println!("latencies       : {latencies:?}"      );
		println!("buffer_size     : {buffer_size:?}"    );
		println!("get_sample_rate : {get_sample_rate:?}");
		println!("sample_position : {sample_position:?}");
		println!();
		
		println!("{:<18}: {:?}", stringify!(EnableTimeCodeRead), driver.future::<EnableTimeCodeRead>(&mut ()).is_ok());
		println!("{:<18}: {:?}", stringify!(CanInputMonitor   ), driver.future::<CanInputMonitor   >(&mut ()).is_ok());
		println!("{:<18}: {:?}", stringify!(CanTimeInfo       ), driver.future::<CanTimeInfo       >(&mut ()).is_ok());
		println!("{:<18}: {:?}", stringify!(CanTimeCode       ), driver.future::<CanTimeCode       >(&mut ()).is_ok());
		println!("{:<18}: {:?}", stringify!(CanTransport      ), driver.future::<CanTransport      >(&mut ()).is_ok());
		println!("{:<18}: {:?}", stringify!(CanInputGain      ), driver.future::<CanInputGain      >(&mut ()).is_ok());
		println!("{:<18}: {:?}", stringify!(CanInputMeter     ), driver.future::<CanInputMeter     >(&mut ()).is_ok());
		println!("{:<18}: {:?}", stringify!(CanOutputGain     ), driver.future::<CanOutputGain     >(&mut ()).is_ok());
		println!("{:<18}: {:?}", stringify!(CanOutputMeter    ), driver.future::<CanOutputMeter    >(&mut ()).is_ok());
		println!("{:<18}: {:?}", stringify!(CanReportOverload ), driver.future::<CanReportOverload >(&mut ()).is_ok());
		let mut io_format = IoFormat {
    		format_type: IoFormatType::DSD,
    		_placeholder: [0; _]
		};
		println!("{} DSD : {:?}", stringify!(CanDoIoFormat), driver.future::<CanDoIoFormat>(&mut io_format).is_ok());
		println!();
		
		println!("channel\tactive\tgroup\tsmpl_ty\tname");
		println!("...................................................");
		
		for i in (0..channels.0).take(16) {
			let channel_info = driver.channel_info(ChannelIndex(i), false)?;
			println!(
				"out {}\t{}\t{}\t{}\t{}",
				i,
				bool::try_from(channel_info.is_active).unwrap(),
				channel_info.group.0,
				channel_info.sample_type.0,
				channel_info.name,
			);
		}
		
		for i in 0..channels.1 {
			let channel_info = driver.channel_info(ChannelIndex(i), true)?;
			println!(
				" in {}\t{}\t{}\t{}\t{}",
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
				"{}\t{}\t{}\t{}\t{}",
				clock_source.index.0,
				clock_source.associated_channel.0,
				clock_source.associated_group.0,
				bool::try_from(clock_source.is_current_source).unwrap(),
				clock_source.name()            
			);
		}
		
		println!();
	}
	
	Ok(())
}