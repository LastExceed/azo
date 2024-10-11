fn main() {
	let all = azo::discover_drivers().unwrap();
	let driver = all[0].create_instance().unwrap();
	
	driver.init(None).unwrap();
	
	println!("name   : {}", driver.name());
	println!("version: {}", driver.version().0);
}