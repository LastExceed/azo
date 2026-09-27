use azo::driver::Driver;
use azo::*;

fn main() {
	let all = driver::Metadata::enumerate().unwrap();
	let driver = driver::SafeHandle::new(&all[0].clsid).unwrap();

	assert!(driver.init(None), "driver failed to initialize");
	let rate = driver.get_sample_rate().unwrap();

	println!("current sample rate: {rate}");
}