//! Reading image files, behind the app's photo interface (record 0010): JPEG, PNG, WebP,
//! TIFF, BMP and GIF, with the camera details and the orientation their EXIF holds; and HEIC,
//! HEIF and AVIF, which the OS decodes (record 0012).

use std::io::Cursor;
use std::path::Path;

use exif::{Exif, In, Tag, Value};
use image::{ImageFormat, ImageReader};

use crate::os_image;
use crate::problem::Problem;
use crate::photo::{has_extension, Orientation, Photo, PhotoInfo, PhotoReader, Preview, Want};
use crate::thumbs;

const FORMATS: &[(&str, ImageFormat)] = &[
    ("jpg", ImageFormat::Jpeg),
    ("jpeg", ImageFormat::Jpeg),
    ("png", ImageFormat::Png),
    ("webp", ImageFormat::WebP),
    ("tif", ImageFormat::Tiff),
    ("tiff", ImageFormat::Tiff),
    ("bmp", ImageFormat::Bmp),
    ("gif", ImageFormat::Gif),
];

/// The formats the OS decodes (record 0012).
const OS_DECODED: &[&str] = &["heic", "heif", "hif", "avif"];
/// The long side of an OS-decoded photo's preview, as a 4K screen shows it whole.
const OS_PREVIEW_SIZE: u32 = 4096;
const OS_PREVIEW_QUALITY: u8 = 90;

fn format_of(path: &Path) -> Option<ImageFormat> {
    let extension = path.extension()?.to_str()?;
    FORMATS.iter().find(|(e, _)| extension.eq_ignore_ascii_case(e)).map(|&(_, format)| format)
}

pub fn is_image(path: &Path) -> bool {
    format_of(path).is_some() || has_extension(path, OS_DECODED)
}

pub struct ImageFileReader;

impl PhotoReader for ImageFileReader {
    fn read(&self, path: &Path, _: Want) -> Result<Photo, Problem> {
        let bytes = std::fs::read(path).map_err(|e| Problem::reading(path, e))?;
        // Many files hold no EXIF: a PNG from an editor, a screenshot. Their details stay empty.
        let exif = exif::Reader::new().read_from_container(&mut Cursor::new(&bytes)).ok();
        let exif = exif.as_ref();
        let info = details(exif);
        if has_extension(path, OS_DECODED) {
            return Ok(read_with_os(&bytes, info));
        }

        let format = format_of(path).ok_or_else(|| format!("{} isn't an image file PhotoMatch reads", path.display()))?;
        let (width, height) = ImageReader::with_format(Cursor::new(&bytes), format).into_dimensions().map_err(|e| decoding_problem(path, e))?;
        let orientation = match exif.and_then(|e| uint(e, Tag::Orientation)) {
            Some(3) => Orientation::Rotate180,
            Some(6) => Orientation::Rotate90,
            Some(8) => Orientation::Rotate270,
            _ => Orientation::Normal,
        };
        let (upright_width, upright_height) = if orientation.is_sideways() { (height, width) } else { (width, height) };
        let info = PhotoInfo { width: upright_width, height: upright_height, preview_width: width, preview_height: height, ..info };
        let preview = Preview { width, height, format, bytes, tagged_orientation: orientation != Orientation::Normal, made: false };
        Ok(Photo { previews: vec![preview], no_preview: None, orientation, info })
    }
}

/// What it means when the image in a file can't be read (plan step 2.35): a file cut short or
/// corrupted is damaged; one in a variant of its format the decoder doesn't handle isn't a photo
/// PhotoMatch reads.
fn decoding_problem(path: &Path, error: image::ImageError) -> Problem {
    let cause = format!("can't read the image in {}: {error}", path.display());
    match error {
        image::ImageError::Decoding(_) | image::ImageError::IoError(_) => Problem::Damaged { cause },
        image::ImageError::Unsupported(_) => Problem::NotAPhoto { cause },
        _ => Problem::Internal { cause },
    }
}

/// HEIC, HEIF and AVIF, decoded by the OS, which turns them upright, so the app doesn't
/// (record 0012). If it can't decode one, the photo keeps its details, with why.
fn read_with_os(bytes: &[u8], info: PhotoInfo) -> Photo {
    let decoded = os_image::decode(bytes, OS_PREVIEW_SIZE)
        .and_then(|d| -> Result<_, Problem> { Ok((thumbs::encode(&image::DynamicImage::ImageRgb8(d.image.clone()), OS_PREVIEW_QUALITY)?, d)) });
    match decoded {
        Ok((jpeg, decoded)) => {
            let (width, height) = decoded.image.dimensions();
            let info = PhotoInfo { width: decoded.width, height: decoded.height, preview_width: width, preview_height: height, ..info };
            let preview = Preview { width, height, format: ImageFormat::Jpeg, bytes: jpeg, tagged_orientation: false, made: true };
            Photo { previews: vec![preview], no_preview: None, orientation: Orientation::Normal, info }
        }
        Err(why) => Photo { previews: Vec::new(), no_preview: Some(why), orientation: Orientation::Normal, info },
    }
}

/// The camera details in a file's EXIF; its sizes are left for the reader to fill in.
fn details(exif: Option<&Exif>) -> PhotoInfo {
    let make = tidy_make(&exif.and_then(|e| text(e, Tag::Make)).unwrap_or_default());
    PhotoInfo {
        model: exif.and_then(|e| text(e, Tag::Model)).map(|m| tidy_model(&m, &make)).unwrap_or_default(),
        make,
        lens: exif.and_then(|e| text(e, Tag::LensModel)).unwrap_or_default(),
        captured_at: exif.and_then(|e| text(e, Tag::DateTimeOriginal).or_else(|| text(e, Tag::DateTime))).and_then(|t| date(&t)),
        iso: exif.and_then(|e| uint(e, Tag::PhotographicSensitivity)).unwrap_or(0),
        shutter_seconds: exif.and_then(|e| rational(e, Tag::ExposureTime)).unwrap_or(0.0),
        aperture: exif.and_then(|e| rational(e, Tag::FNumber)).unwrap_or(0.0),
        focal_length_mm: exif.and_then(|e| rational(e, Tag::FocalLength)).unwrap_or(0.0),
        ..PhotoInfo::default()
    }
}

fn text(exif: &Exif, tag: Tag) -> Option<String> {
    match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Ascii(parts) => {
            let text = String::from_utf8_lossy(parts.first()?).trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string();
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

fn uint(exif: &Exif, tag: Tag) -> Option<u32> {
    exif.get_field(tag, In::PRIMARY)?.value.get_uint(0)
}

fn rational(exif: &Exif, tag: Tag) -> Option<f32> {
    match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Rational(values) => values.first().filter(|r| r.denom != 0).map(|r| r.to_f64() as f32),
        _ => None,
    }
}

/// EXIF's `YYYY:MM:DD HH:MM:SS` as the app writes it, `YYYY-MM-DD HH:MM:SS`.
fn date(exif_date: &str) -> Option<String> {
    let (day, time) = exif_date.split_once(' ')?;
    let day = day.replace(':', "-");
    (day.len() == 10 && !day.starts_with("0000")).then(|| format!("{day} {time}"))
}

/// Makers' names as LibRaw gives them for RAW files, such as "NIKON CORPORATION" as
/// "Nikon", so the filters list each maker once.
fn tidy_make(make: &str) -> String {
    const NAMES: &[(&str, &str)] = &[
        ("apple", "Apple"),
        ("canon", "Canon"),
        ("dji", "DJI"),
        ("fujifilm", "Fujifilm"),
        ("google", "Google"),
        ("hasselblad", "Hasselblad"),
        ("leica", "Leica"),
        ("nikon", "Nikon"),
        ("olympus", "Olympus"),
        ("om digital", "OM Digital"),
        ("panasonic", "Panasonic"),
        ("pentax", "Pentax"),
        ("ricoh", "Ricoh"),
        ("samsung", "Samsung"),
        ("sigma", "Sigma"),
        ("sony", "Sony"),
    ];
    let lower = make.to_lowercase();
    NAMES.iter().find(|(start, _)| lower.starts_with(start)).map_or_else(|| make.trim().to_string(), |(_, name)| name.to_string())
}

/// The model without the maker's name in front, as LibRaw gives it: Canon's "Canon EOS 400D
/// DIGITAL" is "EOS 400D DIGITAL", and Nikon's "NIKON D300" is "D300".
fn tidy_model(model: &str, make: &str) -> String {
    match model.get(..make.len()) {
        Some(start) if !make.is_empty() && start.eq_ignore_ascii_case(make) => model[make.len()..].trim().to_string(),
        _ => model.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makers_and_models_read_as_libraw_gives_them() {
        assert_eq!(tidy_make("NIKON CORPORATION"), "Nikon");
        assert_eq!(tidy_make("SONY"), "Sony");
        assert_eq!(tidy_make("OM Digital Solutions"), "OM Digital");
        assert_eq!(tidy_make("Acme Cameras"), "Acme Cameras");
        assert_eq!(tidy_model("NIKON D300", "Nikon"), "D300");
        assert_eq!(tidy_model("Canon EOS 400D DIGITAL", "Canon"), "EOS 400D DIGITAL");
        assert_eq!(tidy_model("ILCE-7M3", "Sony"), "ILCE-7M3");
        assert_eq!(date("2019:10:06 05:31:48").as_deref(), Some("2019-10-06 05:31:48"));
        assert_eq!(date("0000:00:00 00:00:00"), None);
    }
}
