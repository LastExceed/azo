#![expect(
    non_snake_case,
    trivial_casts,
    clippy::absolute_paths,
    clippy::missing_safety_doc,
    clippy::must_use_candidate,
    clippy::borrow_as_ptr,
    clippy::ptr_as_ptr,
    reason = "generated"
)]

include!(concat!(env!("OUT_DIR"), "/windows_bindgen_output.rs"));