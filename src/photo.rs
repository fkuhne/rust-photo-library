//! Reading photos, RAW or image files, behind an interface of our own (records 0007 and 0010).
//!
//! Nothing outside the readers knows that LibRaw, the `image` crate or EXIF do the work.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::ImageFormat;
use serde::{Deserialize, Serialize};

use crate::image_file::{self, ImageFileReader};
use crate::problem::Problem;
use crate::raw::{self, LibRawReader};

/// What the app needs from a photo file to browse it.
pub struct Photo {
    /// The previews read, smallest first: from a RAW file, only the one the read wanted. A RAW
    /// file holds JPEGs: Sony bodies hold 160×120 and 1616×1080 ones, and newer bodies a
    /// full-size one too. An image file's only preview is the image itself. Empty when the
    /// file holds none the app can show and it can't develop one; the details are there all
    /// the same.
    pub previews: Vec<Preview>,
    /// Why `previews` is empty.
    pub no_preview: Option<Problem>,
    pub orientation: Orientation,
    pub info: PhotoInfo,
}

pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    /// As stored in the file.
    pub bytes: Vec<u8>,
    /// Whether the preview's own EXIF says how to turn it, as an image file's does. Web views
    /// turn such a JPEG by themselves; a RAW file's previews carry no such tag.
    pub tagged_orientation: bool,
    /// Made by the app, developed from RAW data or decoded by the OS, rather than as the file
    /// stores it: worth keeping in the cache once made (plan step 2.26).
    pub made: bool,
}

impl Photo {
    /// The smallest preview whose long side is at least `long_side`, or else the largest; or
    /// why there's none.
    pub fn preview_for(&self, long_side: u32) -> Result<&Preview, Problem> {
        self.previews.iter().find(|p| p.width.max(p.height) >= long_side).or(self.previews.last()).ok_or_else(|| self.why_no_preview())
    }

    pub fn into_largest_preview(mut self) -> Result<Preview, Problem> {
        self.previews.pop().ok_or_else(|| self.why_no_preview())
    }

    fn why_no_preview(&self) -> Problem {
        self.no_preview.clone().unwrap_or_else(|| "it has no preview to show".into())
    }
}

/// How the preview must be turned to stand upright.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
}

impl Orientation {
    pub fn is_sideways(self) -> bool {
        matches!(self, Orientation::Rotate90 | Orientation::Rotate270)
    }
}

/// Where a photo was taken, in degrees, north and east positive (record 0010).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub latitude: f64,
    pub longitude: f64,
}

impl Position {
    /// A position, if it is one: within range, and not 0 and 0, which an empty GPS block gives,
    /// as several cameras write one.
    pub fn new(latitude: f64, longitude: f64) -> Option<Position> {
        let valid = latitude.is_finite() && longitude.is_finite() && latitude.abs() <= 90.0 && longitude.abs() <= 180.0;
        (valid && (latitude != 0.0 || longitude != 0.0)).then_some(Position { latitude, longitude })
    }
}

/// What `PhotoInfo` holds, by version: details cached at an older one are read again, the
/// thumbnail kept (record 0010). 1: the position.
pub const DETAILS_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoInfo {
    pub make: String,
    pub model: String,
    pub lens: String,
    /// As set on the camera's clock, `YYYY-MM-DD HH:MM:SS`.
    pub captured_at: Option<String>,
    pub iso: u32,
    pub shutter_seconds: f32,
    pub aperture: f32,
    pub focal_length_mm: f32,
    /// The photo's size, upright: a portrait photo is taller than wide.
    pub width: u32,
    pub height: u32,
    /// The largest preview's size.
    pub preview_width: u32,
    pub preview_height: u32,
    /// The thumbnail's size, upright, so the grid can give each photo its shape. Set by the
    /// thumbnail cache when it makes one; 0 until then.
    #[serde(default)]
    pub thumb_width: u32,
    #[serde(default)]
    pub thumb_height: u32,
    /// Why the photo can't be shown, when its details could be read but no thumbnail made.
    #[serde(default)]
    pub no_preview: Option<Problem>,
    /// Where it was taken, from the file's GPS; an app may look in its sidecar too,
    /// `sidecar::position` (record 0010).
    #[serde(default)]
    pub position: Option<Position>,
    /// `DETAILS_VERSION` when the details were read; 0 for those cached before it.
    #[serde(default)]
    pub details_version: u32,
}

/// What a read is for, so that a RAW file's reader takes out only the preview it needs, not all
/// of them (record 0016).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    /// The smallest preview whose long side is at least this, or else the largest: for a
    /// thumbnail.
    AtLeast(u32),
    /// The largest preview: for the large view.
    Largest,
    /// The smallest preview: shown for a moment while the thumbnail is made (plan step 2.25).
    Smallest,
    /// The photo at its full resolution, for zooming in: the largest preview if it's about the
    /// photo's own size, or else the RAW data developed at full size (plan step 2.26).
    Full,
    /// No preview, only the details: to read again details cached before what they hold grew,
    /// the thumbnail kept (record 0010).
    Details,
}

pub trait PhotoReader: Send + Sync {
    /// The photo's details, and the preview `want` asks for. An image file is its own preview.
    fn read(&self, path: &Path, want: Want) -> Result<Photo, Problem>;
}

/// Reads RAW files with LibRaw and image files with the `image` crate, by their extension.
pub struct Readers;

impl PhotoReader for Readers {
    fn read(&self, path: &Path, want: Want) -> Result<Photo, Problem> {
        if raw::is_raw(path) {
            LibRawReader.read(path, want)
        } else if image_file::is_image(path) {
            ImageFileReader.read(path, want)
        } else {
            Err(format!("{} isn't a kind of photo PhotoMatch reads", path.display()).into())
        }
    }
}

pub fn is_supported(path: &Path) -> bool {
    raw::is_raw(path) || image_file::is_image(path)
}

/// Whether `path` ends in one of `extensions`, ignoring case.
pub(crate) fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// The photos to show among `files`, in their order, each with the JPEG shot beside it, if
/// any. A JPEG with the same name as a RAW file in the same folder shows once, as the RAW
/// file (record 0011).
pub fn pair(files: Vec<PathBuf>) -> Vec<(PathBuf, Option<PathBuf>)> {
    let key = |p: &Path| (p.parent().map(Path::to_path_buf), p.file_stem().map(|s| s.to_string_lossy().to_lowercase()));
    let is_jpeg = |p: &Path| has_extension(p, &["jpg", "jpeg"]);
    let raws: HashMap<_, ()> = files.iter().filter(|p| raw::is_raw(p)).map(|p| (key(p), ())).collect();
    let mut jpegs: HashMap<_, PathBuf> = files.iter().filter(|p| is_jpeg(p) && raws.contains_key(&key(p))).map(|p| (key(p), p.clone())).collect();
    files
        .into_iter()
        .filter(|p| !(is_jpeg(p) && raws.contains_key(&key(p))))
        .map(|p| {
            let companion = if raw::is_raw(&p) { jpegs.remove(&key(&p)) } else { None };
            (p, companion)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_jpeg_beside_a_raw_file_shows_once_as_the_raw_file() {
        let files = ["/a/DSC1.ARW", "/a/dsc1.JPG", "/a/DSC2.jpg", "/a/DSC3.NEF", "/a/DSC3.tif", "/b/DSC1.jpeg"].map(PathBuf::from).to_vec();
        let shown: Vec<_> = pair(files).into_iter().map(|(p, c)| (p.to_string_lossy().into_owned(), c.map(|c| c.to_string_lossy().into_owned()))).collect();
        assert_eq!(
            shown,
            [
                ("/a/DSC1.ARW".into(), Some("/a/dsc1.JPG".into())),
                ("/a/DSC2.jpg".into(), None),
                ("/a/DSC3.NEF".into(), None),
                ("/a/DSC3.tif".into(), None),
                ("/b/DSC1.jpeg".into(), None),
            ]
        );
    }

    /// An empty GPS block's 0 and 0, and values out of range, aren't a position (record 0010).
    #[test]
    fn only_a_real_position_is_one() {
        assert_eq!(Position::new(37.8, -122.5), Some(Position { latitude: 37.8, longitude: -122.5 }));
        assert_eq!(Position::new(0.0, 0.0), None);
        assert_eq!(Position::new(91.0, 10.0), None);
        assert_eq!(Position::new(10.0, -181.0), None);
        assert_eq!(Position::new(f64::NAN, 10.0), None);
        assert!(Position::new(0.0, 10.0).is_some(), "on the equator");
    }
}
