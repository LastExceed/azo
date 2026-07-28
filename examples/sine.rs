use std::f64::consts::TAU;
use std::ffi::{c_long, c_void};
use std::io::{self, Cursor, Write};
use std::sync::mpsc::TrySendError;
use std::{iter, slice};
use std::sync::mpsc;
use std::sync::OnceLock;
use azo::dto::ChannelId;
use azo::sys::*;

static SENDER: OnceLock<mpsc::SyncSender<c_long>> = OnceLock::new();

fn main() {
	let all = azo::discover_drivers().unwrap();
	let driver = all[3].create_instance().unwrap();

	if let Err(error) = play_sine(&driver) {
		println!("{error} - {:?}", driver.last_error());
	}
}

fn play_sine(driver: &azo::Driver) -> azo::Result<()> {
	let (sender, receiver) = mpsc::sync_channel::<c_long>(2);
	SENDER.set(sender).unwrap();

	driver.init(None)?;
	
	let channel_id = ChannelId { input: false, index: 0 }; // first output channel
	
	let sample_rate = driver.get_sample_rate()?;
	let sample_type = driver.channel_info(channel_id)?.sample_type;
	let buffer_size = driver.buffer_size()?.preferred;	
	let mut sine_wave_generator = create_sine_wave_generator(440.0, sample_rate);
	let (sample_size, fn_write) = match_sample_type(sample_type);
	
	let buf_ptrs =
		unsafe { driver.create_buffers([channel_id], buffer_size, &raw const CALLBACKS) }?
		.next()
		.unwrap();
	
	driver.start()?;
	
	loop {
		let buf_idx = receiver.recv().unwrap() as usize;
		let buf_ptr = buf_ptrs[buf_idx].cast::<u8>();
		let buf_len = buffer_size as usize * sample_size;

		let buf_slice = unsafe { slice::from_raw_parts_mut(buf_ptr, buf_len) };
		let mut cursor = Cursor::new(buf_slice);

		// This is deliberately inefficient.
		// It generates the signal sample by sample, over and over.
		// In praxis, this could be done much more efficiently by pre-rendering the wave once,
		// and then copying a slice of that on each buffer swap,
		// but the goal here is to simulate some form of realtime processing.
		for _ in 0..buffer_size {
			fn_write(sine_wave_generator.next().unwrap(), &mut cursor).unwrap();
		}

		// See the doc-comment of this function for context.
		// Ideally, the exact error code should be checked to prevent missing out on potential other errors,
		// and a mechanism preventing further futile calls could be added as well,
		// but for the sake of simplicity this is neglected here.
		_ = driver.output_ready();
	}
}

fn create_sine_wave_generator(frequency: f64, sample_rate: f64) -> impl Iterator<Item=f64> {
	let mut phase = 0.0;
	
	iter::from_fn(move || {
		let sample = (phase * TAU).sin();

		phase += frequency / sample_rate;
		phase %= 1.0;

		Some(sample)
	})
}

/// Could be cleaned up with macros and/or numtraits, but that would likely make it harder to read
fn match_sample_type<W: Write>(sample_type: SampleType) -> (usize, fn(f64, &mut W) -> io::Result<()>) {
	const I18_MAX: i32 = 0x00_01_FF_FF;
	const I20_MAX: i32 = 0x00_07_FF_FF;
	const I24_MAX: i32 = 0x00_7F_FF_FF;

	match sample_type {
		SampleType::PCM_I16_MSB    => (size_of::<i16>(), |sample, dst| dst.write_all(&((i16::MAX as f64 * sample) as i16).to_be_bytes()     )),
		SampleType::PCM_I24_MSB    => (3               , |sample, dst| dst.write_all(&(( I24_MAX as f64 * sample) as i32).to_be_bytes()[1..])),
		SampleType::PCM_I32_MSB    => (size_of::<i32>(), |sample, dst| dst.write_all(&((i32::MAX as f64 * sample) as i32).to_be_bytes()     )),
		SampleType::PCM_F32_MSB    => (size_of::<f32>(), |sample, dst| dst.write_all(&(                   sample  as f32).to_be_bytes()     )),
		SampleType::PCM_F64_MSB    => (size_of::<f64>(), |sample, dst| dst.write_all(&                    sample         .to_be_bytes()     )),
		SampleType::PCM_I32_MSB_16 => (size_of::<i32>(), |sample, dst| dst.write_all(&((i16::MAX as f64 * sample) as i32).to_be_bytes()     )),
		SampleType::PCM_I32_MSB_18 => (size_of::<i16>(), |sample, dst| dst.write_all(&(( I18_MAX as f64 * sample) as i16).to_be_bytes()     )),
		SampleType::PCM_I32_MSB_20 => (size_of::<i16>(), |sample, dst| dst.write_all(&(( I20_MAX as f64 * sample) as i16).to_be_bytes()     )),
		SampleType::PCM_I32_MSB_24 => (size_of::<i32>(), |sample, dst| dst.write_all(&(( I24_MAX as f64 * sample) as i32).to_be_bytes()     )),
		SampleType::PCM_I16_LSB    => (size_of::<i16>(), |sample, dst| dst.write_all(&((i16::MAX as f64 * sample) as i16).to_le_bytes()     )),
		SampleType::PCM_I24_LSB    => (3               , |sample, dst| dst.write_all(&(( I24_MAX as f64 * sample) as i32).to_le_bytes()[..3])),
		SampleType::PCM_I32_LSB    => (size_of::<i32>(), |sample, dst| dst.write_all(&((i32::MAX as f64 * sample) as i32).to_le_bytes()     )),
		SampleType::PCM_F32_LSB    => (size_of::<f32>(), |sample, dst| dst.write_all(&(                   sample  as f32).to_le_bytes()     )),
		SampleType::PCM_F64_LSB    => (size_of::<f64>(), |sample, dst| dst.write_all(&                    sample         .to_le_bytes()     )),
		SampleType::PCM_I32_LSB_16 => (size_of::<i32>(), |sample, dst| dst.write_all(&((i16::MAX as f64 * sample) as i32).to_le_bytes()     )),
		SampleType::PCM_I32_LSB_18 => (size_of::<i16>(), |sample, dst| dst.write_all(&(( I18_MAX as f64 * sample) as i16).to_le_bytes()     )),
		SampleType::PCM_I32_LSB_20 => (size_of::<i16>(), |sample, dst| dst.write_all(&(( I20_MAX as f64 * sample) as i16).to_le_bytes()     )),
		SampleType::PCM_I32_LSB_24 => (size_of::<i32>(), |sample, dst| dst.write_all(&(( I24_MAX as f64 * sample) as i32).to_le_bytes()     )),
		
		SampleType::DSD_I8_LSB_1 |
		SampleType::DSD_I8_MSB_1 |
		SampleType::DSD_I8_NER_8 => unimplemented!(),
		
		_ => panic!("unknown sample type")
	}
}

static CALLBACKS: Callbacks = Callbacks {
    buffer_switch,
    sample_rate_did_change,
    asio_message,
    buffer_switch_time_info
};

/// for compatibility and simplicity, this example always assumes `direct_process == false`
unsafe extern "system" fn buffer_switch(buffer_index: c_long, _direct_process: Bool) {
	let send_result = SENDER.get().unwrap().try_send(buffer_index);
	
	use TrySendError::*;
	match send_result {
		Ok(()) => return, // nothing more to do here, the rest is up to the processing thread
		
		Err(Full(_)) => return, // processing thread fell behind / froze / died / whatever.
		                        // An underrun is imminent (audible glitches).
			                    // In production, consider handling this scenario more gracefully
								// e.g. by initiating a buffer size increase.

		Err(Disconnected(_)) => panic!("mpsc channel disconnected")
	}
}

unsafe extern "system" fn sample_rate_did_change(_rate: SampleRate) {
	unimplemented!()
}

unsafe extern "system" fn asio_message(
	_selector: MessageSelector,
	_value   : c_long,
	_message : *const c_void,
	_opt     : *const f64
) -> c_long {
	Bool::FALSE.0
}

unsafe extern "system" fn buffer_switch_time_info(
	_params             : *const Time,
	_double_buffer_index: c_long,
	_direct_process     : Bool
) -> Time {
	unimplemented!()
}