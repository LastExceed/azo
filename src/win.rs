#![expect(
    non_snake_case,
    unreachable_pub,
    clippy::pedantic,
    clippy::restriction,
    clippy::blanket_clippy_restriction_lints, // false positive due to above
    reason = "generated"
)]
include!(concat!(env!("OUT_DIR"), "/windows_bindgen_output.rs"));