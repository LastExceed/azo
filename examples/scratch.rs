use std::ffi::{c_long, c_void};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::{ptr, slice, thread};
use azo::dto::ChannelId;
use azo::sys::*;

fn main() {
	let driver =
		azo::get_drivers()
		.unwrap()
		.into_iter()
		.find(|meta| meta.description.to_string_lossy().starts_with("TOPPING"))
		.unwrap()
		.create_instance()
		.unwrap();
	
	assert!(driver.init(None));

	let pref = driver.buffer_size().unwrap().preferred;
	println!("pref: {pref}\n-----");
	
	let buf_ptrs = unsafe { driver.create_buffers([ChannelId { index: 0, input: false }], pref, &raw const CALLBACKS) }
    	.unwrap()
		.next()
		.unwrap();
	
	unsafe { BUF_PTRS = buf_ptrs; }
	
	println!(">> {buf_ptrs:?}");
	println!(">> {}", buf_ptrs[1].addr() - buf_ptrs[0].addr() - BUFFER_LEN_BYTES);
	
	driver.start().unwrap();
	
	thread::park();
}

const SUPPORTED_SELECTORS: &[MessageSelector] = &[
	MessageSelector::SELECTOR_SUPPORTED,
	MessageSelector::ENGINE_VERSION,
	// MessageSelector::RESET_REQUEST,
	// MessageSelector::BUFFER_SIZE_CHANGE,
	// MessageSelector::RESYNC_REQUEST,
	// MessageSelector::LATENCIES_CHANGED,
	MessageSelector::SUPPORTS_TIME_INFO,
	MessageSelector::SUPPORTS_TIME_CODE
];

const SAMPLE_SIZE: usize = 4;
const BUFFER_SIZE: usize = 8;
const BUFFER_LEN_BYTES: usize = SAMPLE_SIZE * BUFFER_SIZE;

const ALLOC_LEN_BYTES: usize = BUFFER_LEN_BYTES * ALLOC_MULT;

const BUFFER_COUNT: usize = 2;
const TOTAL_LEN_BYTES: usize = ALLOC_LEN_BYTES * BUFFER_COUNT;


static N: AtomicU8 = AtomicU8::new(1);
static mut BUF_PTRS: [*mut c_void; 2] = [ptr::null_mut(); 2];

static CALLBACKS: Callbacks = Callbacks {
    buffer_switch,
    sample_rate_did_change,
    asio_message,
    buffer_switch_time_info
};

unsafe extern "system" fn buffer_switch(buffer_index: c_long, _direct_process: Bool) {
	let n = N.fetch_add(1, Ordering::SeqCst);
	println!("n={n}:");
	
	let ptr0 = unsafe { BUF_PTRS[0] }.cast::<[u8; BUFFER_LEN_BYTES]>();
	
	for i_half in 0..BUFFER_COUNT {
		for i_row in 0..ALLOC_MULT {
			let ptr_row = ptr0.add(i_row + i_half * ALLOC_MULT);
			let row = unsafe { *ptr_row };
			println!("{row:x?}");
		}
	}
	
	
	let ptr_target = unsafe { BUF_PTRS[buffer_index as usize] }.cast::<[u8; BUFFER_LEN_BYTES]>();
	
	ptr_target.write_bytes(n, 1);
	
	if n == 10 { panic!() };
}

unsafe extern "system" fn sample_rate_did_change(_rate: SampleRate) {
	unimplemented!()
}

unsafe extern "system" fn asio_message(
	selector: MessageSelector,
	value   : c_long,
	message : *const c_void,
	opt     : *const f64
) -> c_long {
	print!("{:<20}", selector_name(selector));
	if selector == MessageSelector::SELECTOR_SUPPORTED {
		print!("value = {}", selector_name(MessageSelector(value)));
	} else if value != 0 {
		print!("value = {value}");
	} else {
		// skip
	}
	
	if !message.is_null() {
		print!("message = {message:?}");
	}
	
	if !opt.is_null() {
		print!("{}", unsafe { *opt });
	}
	println!();
	

	match selector {
		MessageSelector::SELECTOR_SUPPORTED => {
			Bool::from(SUPPORTED_SELECTORS.contains(&MessageSelector(value))).0
		}
		MessageSelector::ENGINE_VERSION => 2,
		MessageSelector::RESET_REQUEST => Bool::TRUE.0,
		MessageSelector::BUFFER_SIZE_CHANGE => Bool::TRUE.0,
		MessageSelector::RESYNC_REQUEST => Bool::TRUE.0,
		MessageSelector::LATENCIES_CHANGED => Bool::TRUE.0,
		MessageSelector::SUPPORTS_TIME_INFO => Bool::FALSE.0,
    	MessageSelector::SUPPORTS_TIME_CODE => Bool::FALSE.0,
		_ => Bool::FALSE.0
	}
}

unsafe extern "system" fn buffer_switch_time_info(
	_params             : *mut Time,
	_double_buffer_index: c_long,
	_direct_process     : Bool
) -> *mut Time {
	unimplemented!()
}

fn selector_name(selector: MessageSelector) -> &'static str {
	match selector {
		MessageSelector::SELECTOR_SUPPORTED => "SELECTOR_SUPPORTED",
		MessageSelector::ENGINE_VERSION     => "ENGINE_VERSION",
		MessageSelector::RESET_REQUEST      => "RESET_REQUEST",
		MessageSelector::BUFFER_SIZE_CHANGE => "BUFFER_SIZE_CHANGE",
		MessageSelector::RESYNC_REQUEST     => "RESYNC_REQUEST",
		MessageSelector::LATENCIES_CHANGED  => "LATENCIES_CHANGED",
		MessageSelector::SUPPORTS_TIME_INFO => "SUPPORTS_TIME_INFO",
		MessageSelector::SUPPORTS_TIME_CODE => "SUPPORTS_TIME_CODE",
		_ => "unknown"
	}
}