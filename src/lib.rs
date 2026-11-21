//! photo-library: what the owner's photo apps share, so that it exists once (record 0002).
//!
//! Each module is moved from PhotoMatch as it is, with its tests (record 0004): photo reading,
//! thumbnails and problems, and the file writing thumbnails need, in plan step 1.2; folder listing
//! in step 1.3; ratings and XMP sidecars in step 1.4.

/// LibRaw through `rsraw`, with PhotoMatch's and PhotoDev's additions, re-exported so that an app
/// reaches the very copy the library builds, LibRaw 0.22.3 included, and needs no dependency of its
/// own on it (the library's plan step 1.2). PhotoDev calls its processing options directly.
pub use rsraw;
pub use rsraw_sys;

/// Writing a file atomically and durably, through a temporary file.
pub mod files;
/// Image files through the `image` crate, with their EXIF (PhotoMatch's record 0010).
pub mod image_file;
/// HEIC, HEIF and AVIF, decoded by the OS: ImageIO on macOS, WIC on Windows (PhotoMatch's record 0012).
pub mod os_image;
/// The photo interface: previews, orientation, details, which reader, RAW+JPEG pairs (PhotoMatch's record 0007).
pub mod photo;
/// Problems as kinds, for the apps' interfaces to word (PhotoMatch's record 0022).
pub mod problem;
/// RAW files through LibRaw (PhotoMatch's records 0007, 0016 and 0033).
pub mod raw;
/// Thumbnails, details and the large view's images, cached on disk (PhotoMatch's records 0015 and 0034).
pub mod thumbs;

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
