fn main() {
	let all = azo::discover_drivers().unwrap();
	let driver = all[0].create_instance().unwrap();

	driver.init(None).unwrap();
	let rate = driver.get_sample_rate().unwrap();

	println!("current sample rate: {rate}");
}