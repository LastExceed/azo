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
		"S_FALSE",
		"PostThreadMessageW",
		"GetThreadId",
		"WM_APP",
		"ERROR_INVALID_THREAD_ID",
		"ERROR_NOT_ENOUGH_QUOTA",
		"WM_QUIT",
		"CreateWindowExW",
		"DefWindowProcW",
		"DestroyWindow",
		"RegisterClassExW",
		"WNDCLASSEXW",
		"HWND_MESSAGE",
		"GetModuleHandleW",
		"PostMessageW",
		"ERROR_CLASS_ALREADY_EXISTS",
		"GetLastError",
		"MSG"
	])
	.flat()
	.dead_code()
	.output(format!("{out_dir}/windows_bindgen_output.rs"))
	.write();
}