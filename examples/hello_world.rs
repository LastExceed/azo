use azo::*;

fn main() {
	let all = driver::Metadata::enumerate().unwrap();
	let driver = all[0].create_instance().unwrap();

	assert!(driver.init(None), "driver failed to initialize");
	let rate = driver.get_sample_rate().unwrap();

	println!("current sample rate: {rate}");
}