#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::approx_constant,
    clippy::redundant_static_lifetimes,
    non_snake_case
)]
#![no_std]

mod c_api {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use c_api::*;

/// What LibRaw reads but doesn't keep, kept by photo-library's own callbacks as it reads the file
/// (`glue/shot_details.cpp`, its record 0023).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct rsraw_shot_details {
    /// EXIF's SubSecTimeOriginal as written, its digits ended by a 0; empty where none.
    pub subsec_original: [libc::c_char; 16],
}

impl Default for rsraw_shot_details {
    fn default() -> Self {
        Self { subsec_original: [0; 16] }
    }
}

extern "C" {
    /// After `libraw_init` and before a file is opened: `details` is filled as LibRaw reads it, and
    /// must stay where it is until LibRaw's handle is closed.
    pub fn rsraw_watch_shot_details(data: *mut libraw_data_t, details: *mut rsraw_shot_details);
}
