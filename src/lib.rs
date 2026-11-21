//! photo-library: what the owner's photo apps share, so that it exists once (record 0002).
//!
//! Photo reading, thumbnails and problems come in plan step 1.2, folder listing and file writing in
//! step 1.3, and ratings and XMP sidecars in step 1.4, each moved from Photomatch as it is, with its
//! tests (record 0004).

#[cfg(test)]
mod tests {
    /// The library builds LibRaw 0.22.3, from its submodule, and links it (record 0005).
    #[test]
    fn libraw_0_22_3_is_built_and_linked() {
        // SAFETY: LibRaw returns a static, NUL-terminated string.
        let version = unsafe { std::ffi::CStr::from_ptr(rsraw_sys::libraw_version()) };
        assert!(version.to_str().unwrap().starts_with("0.22.3"), "{version:?}");
    }
}
