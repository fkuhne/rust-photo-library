//! HEIC, HEIF and AVIF photos, decoded by the OS's own decoders (record 0012): ImageIO on macOS,
//! WIC on Windows (plan step 2.1, record 0028), where Microsoft's extensions add the formats.

use image::RgbImage;

use crate::problem::Problem;

pub struct Decoded {
    /// The photo, upright, no larger than asked.
    pub image: RgbImage,
    /// The photo's own size, upright.
    pub width: u32,
    pub height: u32,
}

/// Decodes `bytes`, upright, at most `max_side` pixels on its long side, in sRGB.
#[cfg(target_os = "macos")]
pub fn decode(bytes: &[u8], max_side: u32) -> Result<Decoded, Problem> {
    macos::decode(bytes, max_side)
}

#[cfg(target_os = "windows")]
pub fn decode(bytes: &[u8], max_side: u32) -> Result<Decoded, Problem> {
    wic::decode(bytes, max_side)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn decode(_bytes: &[u8], _max_side: u32) -> Result<Decoded, Problem> {
    Err("no decoder for HEIC and AVIF on this OS".into())
}

/// "HEIC" or "AVIF", from the brands of the file's `ftyp` box, for the extension it needs.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn format_of(bytes: &[u8]) -> &'static str {
    let size = bytes.get(..4).map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let brands = bytes.get(8..size.min(bytes.len()).max(8)).unwrap_or_default();
    if bytes.get(4..8) == Some(b"ftyp") && brands.chunks(4).any(|brand| brand == b"avif" || brand == b"avis") { "AVIF" } else { "HEIC" }
}

/// A binding of our own to the few ImageIO, Core Graphics and Core Foundation functions needed.
#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;

    use super::Decoded;
    use crate::problem::Problem;

    type CFTypeRef = *const c_void;
    type CFIndex = isize;

    #[repr(C)]
    struct CGRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        static kCFBooleanTrue: CFTypeRef;
        fn CFDataCreate(allocator: CFTypeRef, bytes: *const u8, length: CFIndex) -> CFTypeRef;
        fn CFNumberCreate(allocator: CFTypeRef, number_type: CFIndex, value: *const c_void) -> CFTypeRef;
        fn CFNumberGetValue(number: CFTypeRef, number_type: CFIndex, value: *mut c_void) -> bool;
        fn CFDictionaryCreate(
            allocator: CFTypeRef,
            keys: *const CFTypeRef,
            values: *const CFTypeRef,
            count: CFIndex,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> CFTypeRef;
        fn CFDictionaryGetValue(dictionary: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        fn CFRelease(object: CFTypeRef);
    }

    #[link(name = "ImageIO", kind = "framework")]
    unsafe extern "C" {
        static kCGImageSourceCreateThumbnailFromImageAlways: CFTypeRef;
        static kCGImageSourceCreateThumbnailWithTransform: CFTypeRef;
        static kCGImageSourceThumbnailMaxPixelSize: CFTypeRef;
        static kCGImagePropertyPixelWidth: CFTypeRef;
        static kCGImagePropertyPixelHeight: CFTypeRef;
        fn CGImageSourceCreateWithData(data: CFTypeRef, options: CFTypeRef) -> CFTypeRef;
        fn CGImageSourceCreateThumbnailAtIndex(source: CFTypeRef, index: usize, options: CFTypeRef) -> CFTypeRef;
        fn CGImageSourceCopyPropertiesAtIndex(source: CFTypeRef, index: usize, options: CFTypeRef) -> CFTypeRef;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        static kCGColorSpaceSRGB: CFTypeRef;
        fn CGColorSpaceCreateWithName(name: CFTypeRef) -> CFTypeRef;
        fn CGImageGetWidth(image: CFTypeRef) -> usize;
        fn CGImageGetHeight(image: CFTypeRef) -> usize;
        fn CGBitmapContextCreate(
            data: *mut c_void,
            width: usize,
            height: usize,
            bits_per_component: usize,
            bytes_per_row: usize,
            space: CFTypeRef,
            bitmap_info: u32,
        ) -> CFTypeRef;
        fn CGContextDrawImage(context: CFTypeRef, rect: CGRect, image: CFTypeRef);
    }

    /// `kCFNumberSInt64Type`.
    const SINT64: CFIndex = 4;
    /// `kCGImageAlphaNoneSkipLast`: 8-bit RGB, each pixel padded to four bytes.
    const RGBX: u32 = 5;

    /// A Core Foundation object, released when dropped.
    struct Owned(CFTypeRef);

    impl Owned {
        fn new(object: CFTypeRef, what: &str) -> Result<Self, String> {
            if object.is_null() { Err(format!("macOS can't {what}")) } else { Ok(Self(object)) }
        }
    }

    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: `self.0` is a non-null object we own, from a Create or Copy function.
            unsafe { CFRelease(self.0) }
        }
    }

    /// A file whose image macOS can't read or decode is taken as damaged; any other failure
    /// is PhotoMatch's own (plan step 2.35).
    pub fn decode(bytes: &[u8], max_side: u32) -> Result<Decoded, Problem> {
        let damaged = |cause: String| Problem::Damaged { cause };
        // SAFETY: each call gets live objects of the types it expects; every object created
        // is released by `Owned`; the bitmap's buffer outlives the context drawing into it.
        unsafe {
            let data = Owned::new(CFDataCreate(std::ptr::null(), bytes.as_ptr(), bytes.len() as CFIndex), "read the file")?;
            let source = Owned::new(CGImageSourceCreateWithData(data.0, std::ptr::null()), "read the image in the file").map_err(damaged)?;

            let max = Owned::new(CFNumberCreate(std::ptr::null(), SINT64, (&(max_side as i64) as *const i64).cast()), "make a number")?;
            let keys = [kCGImageSourceCreateThumbnailFromImageAlways, kCGImageSourceCreateThumbnailWithTransform, kCGImageSourceThumbnailMaxPixelSize];
            let values = [kCFBooleanTrue, kCFBooleanTrue, max.0];
            let options = Owned::new(
                CFDictionaryCreate(std::ptr::null(), keys.as_ptr(), values.as_ptr(), 3, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks),
                "set the decoding options",
            )?;
            let image = Owned::new(CGImageSourceCreateThumbnailAtIndex(source.0, 0, options.0), "decode the image").map_err(damaged)?;
            let (width, height) = (CGImageGetWidth(image.0), CGImageGetHeight(image.0));

            let space = Owned::new(CGColorSpaceCreateWithName(kCGColorSpaceSRGB), "make an sRGB colour space")?;
            let mut rgbx = vec![0u8; width * height * 4];
            let context = Owned::new(CGBitmapContextCreate(rgbx.as_mut_ptr().cast(), width, height, 8, width * 4, space.0, RGBX), "make a bitmap")?;
            CGContextDrawImage(context.0, CGRect { x: 0.0, y: 0.0, width: width as f64, height: height as f64 }, image.0);
            drop(context);

            let rgb: Vec<u8> = rgbx.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
            let image = image::RgbImage::from_raw(width as u32, height as u32, rgb).ok_or("the decoded image has the wrong size")?;

            // The photo's own size, upright: its long side, in the decoded image's shape.
            let properties = Owned::new(CGImageSourceCopyPropertiesAtIndex(source.0, 0, std::ptr::null()), "read the image's properties")?;
            let long_side = number(properties.0, kCGImagePropertyPixelWidth).max(number(properties.0, kCGImagePropertyPixelHeight)).max(width.max(height) as i64) as f64;
            let scale = long_side / width.max(height) as f64;
            Ok(Decoded { image, width: (width as f64 * scale).round() as u32, height: (height as f64 * scale).round() as u32 })
        }
    }

    /// A whole number in a properties dictionary, or 0.
    unsafe fn number(dictionary: CFTypeRef, key: CFTypeRef) -> i64 {
        let mut value = 0i64;
        // SAFETY: `dictionary` is a live CFDictionary; the value it holds isn't ours to release.
        unsafe {
            let number = CFDictionaryGetValue(dictionary, key);
            if !number.is_null() {
                CFNumberGetValue(number, SINT64, (&mut value as *mut i64).cast());
            }
        }
        value
    }
}

/// WIC, through the `windows` crate (record 0028). Windows 10 and 11 decode HEIC with Microsoft's
/// HEIF and HEVC extensions, and AVIF with its AV1 one, from the Microsoft Store.
#[cfg(target_os = "windows")]
mod wic {
    use image::RgbImage;
    use windows::Win32::Foundation::{E_FAIL, WINCODEC_ERR_COMPONENTINITIALIZEFAILURE, WINCODEC_ERR_COMPONENTNOTFOUND};
    use windows::Win32::Graphics::Imaging::*;
    use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
    use windows::Win32::System::Variant::VT_UI2;
    use windows::core::{Error, HRESULT, Interface, w};

    use super::{Decoded, format_of};
    use crate::problem::Problem;

    /// Media Foundation's "no codec": what a HEIC gives with the HEIF extension and no working HEVC
    /// one, as on the owner's PC, where Windows' Photos app couldn't show it either (step 2.1).
    const MF_E_TOPO_CODEC_NOT_FOUND: HRESULT = HRESULT(0xC00D5212_u32 as i32);

    pub fn decode(bytes: &[u8], max_side: u32) -> Result<Decoded, Problem> {
        // SAFETY: COM calls on objects made here, reading `bytes`, which outlive them.
        unsafe { decode_with_wic(bytes, max_side) }.map_err(|error| {
            let cause = format!("Windows can't decode it: {error}");
            let code = error.code();
            if [WINCODEC_ERR_COMPONENTNOTFOUND, WINCODEC_ERR_COMPONENTINITIALIZEFAILURE, MF_E_TOPO_CODEC_NOT_FOUND].contains(&code) {
                let codec_only = code == MF_E_TOPO_CODEC_NOT_FOUND;
                Problem::NeedsWindowsExtension { format: format_of(bytes).into(), codec_only, cause }
            } else {
                Problem::Damaged { cause }
            }
        })
    }

    unsafe fn decode_with_wic(bytes: &[u8], max_side: u32) -> windows::core::Result<Decoded> {
        unsafe {
            // COM on this thread; if it has it already, in either mode, that serves as well.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let stream = factory.CreateStream()?;
            stream.InitializeFromMemory(bytes)?;
            let decoder = factory.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)?;
            let frame = decoder.GetFrame(0)?;
            let (mut width, mut height) = (0, 0);
            frame.GetSize(&mut width, &mut height)?;
            let (transform, sideways) = turn(orientation(&frame));

            let mut source: IWICBitmapSource = frame.cast()?;
            let long_side = width.max(height);
            if long_side > max_side {
                let scale = |side: u32| ((side as u64 * max_side as u64 + long_side as u64 / 2) / long_side as u64).max(1) as u32;
                let scaler = factory.CreateBitmapScaler()?;
                scaler.Initialize(&source, scale(width), scale(height), WICBitmapInterpolationModeHighQualityCubic)?;
                source = scaler.cast()?;
            }
            if transform != WICBitmapTransformRotate0 {
                let rotator = factory.CreateBitmapFlipRotator()?;
                rotator.Initialize(&source, transform)?;
                source = rotator.cast()?;
            }
            // The photo's own colours made sRGB, as ImageIO makes them on a Mac; as stored if
            // Windows can't.
            if let Some(srgb) = to_srgb(&factory, &frame, &source) {
                source = srgb;
            }
            let converter = factory.CreateFormatConverter()?;
            converter.Initialize(&source, &GUID_WICPixelFormat24bppRGB, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
            let (mut out_width, mut out_height) = (0, 0);
            converter.GetSize(&mut out_width, &mut out_height)?;
            let stride = out_width * 3;
            let mut pixels = vec![0u8; stride as usize * out_height as usize];
            converter.CopyPixels(std::ptr::null(), stride, &mut pixels)?;
            let image = RgbImage::from_raw(out_width, out_height, pixels).ok_or_else(|| Error::from(E_FAIL))?;
            let (width, height) = if sideways { (height, width) } else { (width, height) };
            Ok(Decoded { image, width, height })
        }
    }

    /// The EXIF orientation Windows reads for the photo, 1 if it reads none.
    unsafe fn orientation(frame: &IWICBitmapFrameDecode) -> u16 {
        unsafe {
            let Ok(reader) = frame.GetMetadataQueryReader() else { return 1 };
            let mut value = PROPVARIANT::default();
            if reader.GetMetadataByName(w!("System.Photo.Orientation"), &mut value).is_err() {
                return 1;
            }
            let inner = &value.Anonymous.Anonymous;
            let orientation = if inner.vt == VT_UI2 { inner.Anonymous.uiVal } else { 1 };
            let _ = PropVariantClear(&mut value);
            orientation
        }
    }

    /// WIC's transform for an EXIF orientation, and whether it turns the photo on its side; as
    /// DirectXTex's WIC loader maps them (unchecked for 5 and 7, which no sample has).
    fn turn(orientation: u16) -> (WICBitmapTransformOptions, bool) {
        match orientation {
            2 => (WICBitmapTransformFlipHorizontal, false),
            3 => (WICBitmapTransformRotate180, false),
            4 => (WICBitmapTransformFlipVertical, false),
            5 => (WICBitmapTransformOptions(WICBitmapTransformRotate270.0 | WICBitmapTransformFlipHorizontal.0), true),
            6 => (WICBitmapTransformRotate90, true),
            7 => (WICBitmapTransformOptions(WICBitmapTransformRotate90.0 | WICBitmapTransformFlipHorizontal.0), true),
            8 => (WICBitmapTransformRotate270, true),
            _ => (WICBitmapTransformRotate0, false),
        }
    }

    /// `source` in sRGB, from the frame's own colour profile; `None` if it has none, or if WIC
    /// can't convert it.
    unsafe fn to_srgb(factory: &IWICImagingFactory, frame: &IWICBitmapFrameDecode, source: &IWICBitmapSource) -> Option<IWICBitmapSource> {
        unsafe {
            let mut contexts = [Some(factory.CreateColorContext().ok()?)];
            let mut count = 0;
            frame.GetColorContexts(&mut contexts, &mut count).ok()?;
            if count == 0 {
                return None;
            }
            let srgb = factory.CreateColorContext().ok()?;
            srgb.InitializeFromExifColorSpace(1).ok()?;
            let transform = factory.CreateColorTransformer().ok()?;
            transform.Initialize(source, contexts[0].as_ref(), &srgb, &GUID_WICPixelFormat24bppBGR).ok()?;
            transform.cast().ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_of;

    /// An `ftyp` box: its size, its type, a major brand, a version, then compatible brands.
    fn ftyp(major: &[u8; 4], compatible: &[&[u8; 4]]) -> Vec<u8> {
        let size = 16 + 4 * compatible.len() as u32;
        let mut bytes = size.to_be_bytes().to_vec();
        bytes.extend_from_slice(b"ftyp");
        bytes.extend_from_slice(major);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        compatible.iter().for_each(|brand| bytes.extend_from_slice(*brand));
        bytes.extend_from_slice(b"\0\0\0\x08meta");
        bytes
    }

    #[test]
    fn a_file_says_whether_it_needs_the_heif_or_the_av1_extension() {
        assert_eq!(format_of(&ftyp(b"heic", &[b"mif1", b"heic"])), "HEIC");
        assert_eq!(format_of(&ftyp(b"avif", &[b"mif1", b"miaf"])), "AVIF");
        assert_eq!(format_of(&ftyp(b"mif1", &[b"avif", b"miaf"])), "AVIF");
        assert_eq!(format_of(b"not a photo"), "HEIC");
    }
}
