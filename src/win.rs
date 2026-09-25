#![expect(
    non_snake_case,
    unreachable_pub,
    clippy::pedantic,
    clippy::restriction,
    clippy::blanket_clippy_restriction_lints, // false positive due to above
    reason = "generated"
)]

#[expect(unused_imports, reason = "false positive")]
pub use HWND;

include!(concat!(env!("OUT_DIR"), "/windows_bindgen_output.rs"));

pub type Result<T> = windows_core::Result<T>;