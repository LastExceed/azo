use std::env;

fn main() {
	let out_dir = env::var("OUT_DIR").expect("env var `OUT_DIR` should be set by cargo");

	windows_bindgen::bindgen([
		"--out", &format!("{out_dir}/windows_bindgen_out.rs"),
		"--flat",
		"--filter",
		"CoInitializeEx",
		"COINIT_APARTMENTTHREADED",
		"CoCreateInstance",
		"CLSCTX_SERVER",
		"E_POINTER",
		"HWND",
		"CoUninitialize"
	]);
}