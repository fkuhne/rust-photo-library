use std::borrow::Cow;

use chrono::{DateTime, Local, TimeZone};
use rsraw_sys as sys;

use crate::{
    err::{Error, Result},
    processed::ProcessedImage,
    GpsInfo, LensInfo, ThumbListing, ThumbnailImage, Thumbnails,
};

pub type BitDepth = u32;

pub const BIT_DEPTH_8: BitDepth = 8;
pub const BIT_DEPTH_16: BitDepth = 16;

pub struct RawImage {
    raw_data: *mut sys::libraw_data_t,
}

unsafe impl Sync for RawImage {}

unsafe impl Send for RawImage {}

impl RawImage {
    /// LibRaw keeps a pointer into `buf` until the image is dropped: the caller keeps `buf`
    /// alive that long. (PhotoMatch: LibRaw's handle is closed if opening fails.)
    pub fn open(buf: &[u8]) -> Result<Self> {
        let image = Self { raw_data: unsafe { sys::libraw_init(0) } };
        Error::check(unsafe {
            sys::libraw_open_buffer(image.raw_data, buf.as_ptr() as *const _, buf.len())
        })?;
        Ok(image)
    }

    /// Opens a RAW file by its path, through LibRaw's own reader, which reads only what it's
    /// asked for: the metadata, then each preview or the RAW data when it's unpacked, rather
    /// than the whole file into memory first. (Added for PhotoMatch, plan step 2.24.)
    #[cfg(unix)]
    pub fn open_file(path: &std::path::Path) -> Result<Self> {
        use std::os::unix::ffi::OsStrExt;
        let path = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::Io)?;
        let image = Self { raw_data: unsafe { sys::libraw_init(0) } };
        Error::check(unsafe { sys::libraw_open_file(image.raw_data, path.as_ptr()) })?;
        Ok(image)
    }

    /// The same on Windows, through LibRaw's wide-character call, so that a path with any
    /// letter opens, those outside Windows' code page too. LibRaw has it when built with
    /// Microsoft's compiler, which defines `LIBRAW_WIN32_UNICODEPATHS`. (Added for PhotoMatch,
    /// plan step 2.2, record 0031.)
    #[cfg(windows)]
    pub fn open_file(path: &std::path::Path) -> Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(Error::Io);
        }
        wide.push(0);
        let image = Self { raw_data: unsafe { sys::libraw_init(0) } };
        Error::check(unsafe { sys::libraw_open_wfile(image.raw_data, wide.as_ptr()) })?;
        Ok(image)
    }

    pub fn unpack(&mut self) -> Result<()> {
        unsafe {
            let raw_param = &mut (*self.raw_data).rawparams;
            raw_param.use_rawspeed = 1;
            raw_param.max_raw_memory_mb = 1024;
            Error::check(sys::libraw_unpack(self.raw_data))
        }
    }

    pub fn extract_thumbs(&mut self) -> Result<Vec<ThumbnailImage>> {
        let mut thumbs = Thumbnails::new();
        for i in 0..self.as_ref().thumbs_list.thumbcount {
            thumbs.append(self.extract_thumb(i as usize)?);
        }
        Ok(thumbs.into_inner())
    }

    /// The previews the file holds, as LibRaw lists them, before any is taken out: a size of
    /// 0 × 0 is one LibRaw doesn't know. (Added for PhotoMatch, plan step 2.24.)
    pub fn list_thumbs(&self) -> Vec<ThumbListing> {
        let list = &self.as_ref().thumbs_list;
        list.thumblist[..list.thumbcount.clamp(0, list.thumblist.len() as i32) as usize]
            .iter()
            .map(|t| ThumbListing {
                width: t.twidth as _,
                height: t.theight as _,
                jpeg: t.tformat == sys::LibRaw_internal_thumbnail_formats_LIBRAW_INTERNAL_THUMBNAIL_JPEG,
                length: t.tlength as _,
            })
            .collect()
    }

    /// Takes out the preview at `index` of `list_thumbs`, reading only that from a file opened
    /// with `open_file`. (Added for PhotoMatch, plan step 2.24.)
    pub fn extract_thumb(&mut self, index: usize) -> Result<ThumbnailImage> {
        Error::check(unsafe { sys::libraw_unpack_thumb_ex(self.raw_data, index as _) })?;
        let thumb = &self.as_ref().thumbnail;
        Ok(ThumbnailImage {
            format: thumb.tformat.into(),
            width: thumb.twidth as _,
            height: thumb.theight as _,
            colors: thumb.tcolors as _,
            data: unsafe {
                std::slice::from_raw_parts(thumb.thumb as *const u8, thumb.tlength as _).to_vec()
            },
        })
    }

    /// Enables or disables the ultra-fast pixel binning output.
    /// Must be called before `unpack()`.
    pub fn set_half_size(&mut self, enable: bool) {
        // Because we are touching a C-struct, Rust requires an unsafe block.
        // We are telling the compiler "I know what I'm doing with this pointer."
        unsafe {
            // Find what the underlying C pointer is named in their struct.
            // It might be `self.inner`, `self.libraw_data`, or `self.ptr`.
            // We access the `params` struct and flip the `half_size` integer.
            (*self.raw_data).params.half_size = if enable { 1 } else { 0 };
        }
    }

    pub fn width(&self) -> u32 {
        self.as_ref().sizes.width as _
    }

    pub fn height(&self) -> u32 {
        self.as_ref().sizes.height as _
    }

    pub fn pixels(&self) -> u32 {
        self.width() * self.height()
    }

    pub fn colors(&self) -> i32 {
        self.as_ref().idata.colors
    }

    pub fn iso_speed(&self) -> u32 {
        self.as_ref().other.iso_speed as _
    }

    pub fn shutter(&self) -> f32 {
        self.as_ref().other.shutter as _
    }

    pub fn aperture(&self) -> f32 {
        self.as_ref().other.aperture as _
    }

    pub fn focal_len(&self) -> f32 {
        self.as_ref().other.focal_len as _
    }

    pub fn datetime(&self) -> Option<DateTime<Local>> {
        let ts = self.as_ref().other.timestamp;
        Local.timestamp_opt(i64::from(ts), 0).single()
    }

    pub fn gps(&self) -> GpsInfo {
        self.as_ref().other.parsed_gps.into()
    }

    pub fn artist(&self) -> Cow<'_, str> {
        unsafe {
            std::ffi::CStr::from_ptr(&self.as_ref().other.artist[0] as *const _).to_string_lossy()
        }
    }

    pub fn desc(&self) -> Cow<'_, str> {
        unsafe {
            std::ffi::CStr::from_ptr(&self.as_ref().other.desc[0] as *const _).to_string_lossy()
        }
    }

    pub fn make(&self) -> Cow<'_, str> {
        unsafe {
            std::ffi::CStr::from_ptr(&self.as_ref().idata.make[0] as *const _).to_string_lossy()
        }
    }

    pub fn model(&self) -> Cow<'_, str> {
        unsafe {
            std::ffi::CStr::from_ptr(&self.as_ref().idata.model[0] as *const _).to_string_lossy()
        }
    }

    pub fn normalized_make(&self) -> Cow<'_, str> {
        unsafe {
            std::ffi::CStr::from_ptr(&self.as_ref().idata.normalized_make[0] as *const _)
                .to_string_lossy()
        }
    }

    pub fn normalized_model(&self) -> Cow<'_, str> {
        unsafe {
            std::ffi::CStr::from_ptr(&self.as_ref().idata.normalized_model[0] as *const _)
                .to_string_lossy()
        }
    }

    pub fn software(&self) -> Cow<'_, str> {
        unsafe {
            std::ffi::CStr::from_ptr(&self.as_ref().idata.software[0] as *const _).to_string_lossy()
        }
    }

    pub fn raw_count(&self) -> u32 {
        self.as_ref().idata.raw_count as _
    }

    pub fn dng_version(&self) -> u32 {
        self.as_ref().idata.dng_version as _
    }

    pub fn lens_info(&self) -> LensInfo {
        (&self.as_ref().lens).into()
    }

    pub fn full_info(&self) -> FullRawInfo {
        FullRawInfo {
            width: self.width(),
            height: self.height(),
            colors: self.colors(),
            iso_speed: self.iso_speed(),
            shutter: self.shutter(),
            aperture: self.aperture(),
            focal_len: self.focal_len(),
            datetime: self.datetime(),
            gps: self.gps(),
            artist: self.artist().to_string(),
            desc: self.desc().trim().into(),
            make: self.make().to_string(),
            model: self.model().to_string(),
            normalized_make: self.normalized_make().to_string(),
            normalized_model: self.normalized_model().to_string(),
            software: self.software().to_string(),
            raw_count: self.raw_count(),
            dng_version: self.dng_version(),
            lens_info: self.lens_info(),
        }
    }

    /// If possible, use the white balance from the camera.
    /// If not available, Auto-WB is used.
    pub fn set_use_camera_wb(&mut self, enable: bool) {
        unsafe {
            (*self.raw_data).params.use_camera_wb = if enable { 1 } else { 0 };
        }
    }

    /// On by default, call if you want to force use of DNG embedded matrix.
    pub fn set_use_camera_matrix(&mut self, enable: bool) {
        unsafe {
            (*self.raw_data).params.use_camera_matrix = if enable { 3 } else { 0 };
        }
    }

    pub fn process<const D: BitDepth>(&mut self) -> Result<ProcessedImage<D>> {
        debug_assert!(D == BIT_DEPTH_8 || D == BIT_DEPTH_16);
        unsafe { (*self.raw_data).params.output_bps = D as i32 };
        Error::check(unsafe { sys::libraw_dcraw_process(self.raw_data) })?;

        let mut result = 0i32;
        let processed = unsafe { sys::libraw_dcraw_make_mem_image(self.raw_data, &mut result) };
        Error::check(result)?;
        Ok(unsafe { ProcessedImage::from_raw(processed) })
    }

    // ---- Added for PhotoDev (plan step 1.2, record 0011): LibRaw's processing options, and its
    // ---- processing and TIFF writer as separate calls, so that each stage can be timed.

    /// The output colour space, LibRaw's `output_color`: 0 raw, 1 sRGB, 2 Adobe RGB, 3 Wide,
    /// 4 ProPhoto, 5 XYZ, 6 ACES, 7 DCI-P3, 8 Rec. 2020. (PhotoDev.)
    pub fn set_output_color(&mut self, space: i32) {
        unsafe { sys::libraw_set_output_color(self.raw_data, space) }
    }

    /// Bits a channel in the output, 8 or 16. (PhotoDev.)
    pub fn set_output_bps(&mut self, bits: i32) {
        unsafe { sys::libraw_set_output_bps(self.raw_data, bits) }
    }

    /// The output's transfer curve, LibRaw's `gamm[0]` and `gamm[1]`: the inverted power and
    /// the toe's slope. sRGB's is 1/2.4 and 12.92. (PhotoDev.)
    pub fn set_gamma(&mut self, inverted_power: f32, toe_slope: f32) {
        unsafe {
            sys::libraw_set_gamma(self.raw_data, 0, inverted_power);
            sys::libraw_set_gamma(self.raw_data, 1, toe_slope);
        }
    }

    /// Turns LibRaw's automatic brightening by histogram off (`true`) or on. (PhotoDev.)
    pub fn set_no_auto_bright(&mut self, off: bool) {
        unsafe { sys::libraw_set_no_auto_bright(self.raw_data, off as i32) }
    }

    /// The highlight mode: 0 clip, 1 unclip, 2 blend, 3 to 9 rebuild. (PhotoDev.)
    pub fn set_highlight(&mut self, mode: i32) {
        unsafe { sys::libraw_set_highlight(self.raw_data, mode) }
    }

    /// The demosaic, LibRaw's `user_qual`: 0 linear, 1 VNG, 2 PPG, 3 AHD, 4 DCB, 11 DHT,
    /// 12 modified AHD. (PhotoDev.)
    pub fn set_demosaic(&mut self, method: i32) {
        unsafe { sys::libraw_set_demosaic(self.raw_data, method) }
    }

    /// Develops the unpacked image with LibRaw's own processing, `dcraw_process`, with the
    /// options set above. (PhotoDev.)
    pub fn dcraw_process(&mut self) -> Result<()> {
        Error::check(unsafe { sys::libraw_dcraw_process(self.raw_data) })
    }

    /// The developed image's width and height, after `dcraw_process`, turned as the camera
    /// says, as LibRaw's own `get_mem_image_format` works them out. (PhotoDev.)
    pub fn output_size(&self) -> (u32, u32) {
        let sizes = &self.as_ref().sizes;
        let (width, height) = (sizes.width as u32, sizes.height as u32);
        if sizes.flip & 4 != 0 { (height, width) } else { (width, height) }
    }

    /// Writes the developed image as a TIFF, with LibRaw's own writer, which embeds the output
    /// space's ICC profile. The path must be valid UTF-8; on Windows, LibRaw takes it as a
    /// narrow string, so letters outside ASCII may fail. (PhotoDev.)
    pub fn write_tiff(&mut self, path: &std::path::Path) -> Result<()> {
        let path = path.to_str().ok_or(Error::Io)?;
        let path = std::ffi::CString::new(path).map_err(|_| Error::Io)?;
        unsafe { sys::libraw_set_output_tif(self.raw_data, 1) };
        Error::check(unsafe { sys::libraw_dcraw_ppm_tiff_writer(self.raw_data, path.as_ptr()) })
    }

    /// The camera's colour data, once opened: the multipliers as shot (`cam_mul`), daylight's
    /// (`pre_mul`), and the matrix from XYZ to the camera's RGB (`cam_xyz`), as LibRaw gives
    /// them. (PhotoDev, plan step 1.5.)
    pub fn color_data(&self) -> ([f32; 4], [f32; 4], [[f32; 3]; 4]) {
        let color = &self.as_ref().color;
        (color.cam_mul, color.pre_mul, color.cam_xyz)
    }

    /// A DNG's BaselineExposure, in stops, if the file has one: what its maker asks a converter
    /// to add before rendering. LibRaw marks a file without one with -999. (PhotoDev, plan step
    /// 1.5.)
    pub fn dng_baseline_exposure(&self) -> Option<f32> {
        let value = self.as_ref().color.dng_levels.baseline_exposure;
        (value > -998.0).then_some(value)
    }

    /// The decoder LibRaw picked for the file, once opened, and whether it can decode with it:
    /// some it names but can't, such as Nikon's High Efficiency NEFs, `nikon_he_load_raw()`.
    /// (PhotoDev, plan step 2.1.)
    pub fn decoder(&mut self) -> Option<(String, bool)> {
        let mut info = sys::libraw_decoder_info_t { decoder_name: std::ptr::null(), decoder_flags: 0 };
        if unsafe { sys::libraw_get_decoder_info(self.raw_data, &mut info) } != 0 {
            return None;
        }
        let name = if info.decoder_name.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(info.decoder_name) }.to_string_lossy().into_owned()
        };
        let unsupported = info.decoder_flags & sys::LibRaw_decoder_flags_LIBRAW_DECODER_UNSUPPORTED_FORMAT as u32 != 0;
        Some((name, !unsupported))
    }

    /// LibRaw's version, such as "0.22.3-Release". (PhotoDev.)
    pub fn libraw_version() -> String {
        unsafe { std::ffi::CStr::from_ptr(sys::libraw_version()) }.to_string_lossy().into_owned()
    }
}

impl Drop for RawImage {
    fn drop(&mut self) {
        unsafe { sys::libraw_close(self.raw_data) }
    }
}

impl AsRef<sys::libraw_data_t> for RawImage {
    fn as_ref(&self) -> &sys::libraw_data_t {
        unsafe { &*self.raw_data }
    }
}

impl AsMut<sys::libraw_data_t> for RawImage {
    fn as_mut(&mut self) -> &mut sys::libraw_data_t {
        unsafe { &mut *self.raw_data }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FullRawInfo {
    pub width: u32,
    pub height: u32,
    pub colors: i32,
    pub iso_speed: u32,
    pub shutter: f32,
    pub aperture: f32,
    pub focal_len: f32,
    pub datetime: Option<DateTime<Local>>,
    pub gps: GpsInfo,
    pub artist: String,
    pub desc: String,
    pub make: String,
    pub model: String,
    pub normalized_make: String,
    pub normalized_model: String,
    pub software: String,
    pub raw_count: u32,
    pub dng_version: u32,
    pub lens_info: LensInfo,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rsraw_sys::{
        LibRaw_camera_mounts_LIBRAW_MOUNT_Nikon_Z, LibRaw_camera_mounts_LIBRAW_MOUNT_Sony_E,
    };

    use super::*;
    use crate::{lens::FocusType, processed::ImageFormat, Mounts};

    fn get_test_assets_path() -> PathBuf {
        let root: PathBuf = std::env::var_os("CARGO_MANIFEST_DIR")
            .expect("must get manifest dir")
            .into();
        root.join("tests/assets")
    }

    #[test]
    fn test_raw_metadata() {
        let assets = get_test_assets_path();

        let test_cases = [
            (
                "test-z8.NEF",
                FullRawInfo {
                    width: 8280,
                    height: 5520,
                    colors: 3,
                    iso_speed: 250,
                    shutter: 1.0 / 100.0,
                    aperture: 3.5,
                    focal_len: 105.,
                    datetime: Local.with_ymd_and_hms(2024, 11, 4, 20, 11, 38).single(),
                    // Z8 NEF has GPS EXIF tags present but coordinates are
                    // zeroed (no fix). LibRaw still sets gpsparsed = 1, but
                    // since all coords are zero the result equals default.
                    gps: GpsInfo::default(),
                    artist: "HEXILEE".into(),
                    desc: "".into(),
                    make: "Nikon".into(),
                    model: "Z 8".into(),
                    normalized_make: "Nikon".into(),
                    normalized_model: "Z 8".into(),
                    software: "Ver.02.00".into(),
                    raw_count: 1,
                    dng_version: 0,
                    lens_info: LensInfo {
                        min_focal: 105.0,
                        max_focal: 105.0,
                        max_aperture_at_min_focal: 2.8,
                        max_aperture_at_max_focal: 2.8,
                        lens_make: "NIKON".into(),
                        lens_name: "NIKKOR Z MC 105mm f/2.8 VR S".into(),
                        lens_serial: "20044280".into(),
                        internal_lens_serial: "".into(),
                        focal_length_in_35mm_format: 105,
                        mounts: Mounts::from(LibRaw_camera_mounts_LIBRAW_MOUNT_Nikon_Z).to_string(),
                        focus_type: FocusType::Prime,
                        feture_pre: "AF".into(),
                        feture_suf: "".into(),
                    },
                },
            ),
            (
                "test-a7rm4.ARW",
                FullRawInfo {
                    width: 9568,
                    height: 6376,
                    colors: 3,
                    iso_speed: 320,
                    shutter: 1.0 / 500.0,
                    aperture: 4.0,
                    focal_len: 40.,
                    datetime: Local.with_ymd_and_hms(2023, 11, 17, 13, 0, 13).single(),
                    gps: Default::default(),
                    artist: "hexilee".into(),
                    desc: "".into(),
                    make: "Sony".into(),
                    model: "ILCE-7RM4".into(),
                    normalized_make: "Sony".into(),
                    normalized_model: "ILCE-7RM4".into(),
                    software: "ILCE-7RM4 v1.20".into(),
                    raw_count: 1,
                    dng_version: 0,
                    lens_info: LensInfo {
                        min_focal: 40.0,
                        max_focal: 40.0,
                        max_aperture_at_min_focal: 1.4,
                        max_aperture_at_max_focal: 1.4,
                        lens_make: "".into(),
                        lens_name: "40mm F1.4 DG HSM | Art 018".into(),
                        lens_serial: "".into(),
                        internal_lens_serial: "".into(),
                        focal_length_in_35mm_format: 40,
                        mounts: Mounts::from(LibRaw_camera_mounts_LIBRAW_MOUNT_Sony_E).to_string(),
                        focus_type: FocusType::Prime,
                        feture_pre: "".into(),
                        feture_suf: "".into(),
                    },
                },
            ),
        ];
        for (file, expected) in test_cases {
            let path = assets.join(file);
            let data = std::fs::read(path).unwrap();
            let raw_image = RawImage::open(&data).expect("opened");
            assert!(!raw_image.raw_data.is_null());
            let full_info = raw_image.full_info();
            assert_eq!(full_info, expected);
        }
    }

    #[test]
    fn test_thumbnails() {
        let assets = get_test_assets_path();
        let test_cases = [("test-z8.NEF",), ("test-a7rm4.ARW",)];
        for (file,) in test_cases {
            let path = assets.join(file);
            println!("{path:?}");
            let data = std::fs::read(path).unwrap();
            let mut raw_image = RawImage::open(&data).expect("opened");
            let thumbs = raw_image.extract_thumbs().expect("extracted");
            println!("{:?}", thumbs);
        }
    }

    #[test]
    fn test_processed() {
        let assets = get_test_assets_path();
        let test_cases = [
            (
                "test-z8.NEF",
                8280,
                5520,
                ImageFormat::Bitmap,
                3,
                16,
                274233600,
            ),
            (
                "test-a7rm4.ARW",
                9568,
                6376,
                ImageFormat::Bitmap,
                3,
                16,
                366033408,
            ),
        ];
        for (file, width, height, format, colors, bits, data_size) in test_cases {
            let path = assets.join(file);
            println!("{path:?}");
            let data = std::fs::read(path).unwrap();
            let mut raw_image = RawImage::open(&data).expect("opened");
            raw_image.unpack().expect("unpacked");
            let image = raw_image.process::<BIT_DEPTH_16>().expect("decoded");
            assert_eq!(image.width(), width);
            assert_eq!(image.height(), height);
            assert_eq!(image.image_format(), format);
            assert_eq!(image.colors(), colors);
            assert_eq!(image.bits(), bits);
            assert_eq!(image.data_size(), data_size);
        }
    }
}
