use std::env;

fn main() {
	let out_dir = env::var("OUT_DIR").expect("env var `OUT_DIR` should be set by cargo");

	windows_bindgen
	::builder()
	.filters([
		"CoInitializeEx",
		"COINIT_APARTMENTTHREADED",
		"CoCreateInstance",
		"CLSCTX_SERVER",
		"E_POINTER",
		"HWND",
		"CoUninitialize",
		"GetMessageW",
		"DispatchMessageW",
		"S_OK",
		"PostThreadMessageW",
		"GetThreadId",
		"WM_USER",
		"ERROR_INVALID_THREAD_ID",
		"ERROR_NOT_ENOUGH_QUOTA",
		"WM_QUIT"
	])
	.flat()
	.dead_code()
	.output(format!("{out_dir}/windows_bindgen_output.rs"))
	.write();
}