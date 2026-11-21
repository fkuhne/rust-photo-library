//! XMP sidecars beside RAW files, as Lightroom Classic and Capture One read them (record 0014).
//!
//! Only the pick, the stars and the label are ours. Everything else a sidecar holds, such as
//! Lightroom's edits, is kept as it is. A sidecar PhotoMatch can't read is never written over.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use xmp_toolkit::{FromStrOptions, ToStringOptions, XmpDateTime, XmpMeta, XmpValue, xmp_gps, xmp_ns};

use crate::files::write_durably;
use crate::photo::{has_extension, Position};
use crate::ratings::{Label, Pick, Rating};
use crate::raw;

/// Where Lightroom Classic 13.2 and later keep their flags (`pick` and `good`).
const XMP_DM: &str = "http://ns.adobe.com/xmp/1.0/DynamicMedia/";

/// Our labels as Lightroom's and Capture One's sidecars name them.
const LABELS: [(Label, &str); 4] = [(Label::Red, "Red"), (Label::Yellow, "Yellow"), (Label::Green, "Green"), (Label::Blue, "Blue")];

/// Whether a photo gets a sidecar: a RAW file, but not a DNG, whose sidecar Lightroom would let
/// override the edits inside it. An image file gets none: Lightroom ignores a JPEG's sidecar.
pub fn has_sidecar(photo: &Path) -> bool {
    raw::is_raw(photo) && !has_extension(photo, &["dng"])
}

/// The sidecar of `photo`: `DSC01234.xmp` beside `DSC01234.ARW`, or an existing `DSC01234.XMP`.
pub fn path_for(photo: &Path) -> PathBuf {
    let path = photo.with_extension("xmp");
    let upper = photo.with_extension("XMP");
    if !path.exists() && upper.exists() { upper } else { path }
}

/// A sidecar's modification time and size, to tell whether someone else changed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stamp {
    /// Milliseconds since 1970.
    pub modified_ms: u64,
    pub len: u64,
}

/// Why a sidecar can't be read or written (plan step 2.35): one PhotoMatch can't understand is
/// left as it is; otherwise the disk refused, as a locked card or a read-only folder does.
#[derive(Debug)]
pub enum SidecarError {
    Damaged(String),
    Disk(String),
}

impl std::fmt::Display for SidecarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SidecarError::Damaged(cause) | SidecarError::Disk(cause) => f.write_str(cause),
        }
    }
}

/// A failure to write, or to make the XMP to write.
impl From<String> for SidecarError {
    fn from(cause: String) -> Self {
        SidecarError::Disk(cause)
    }
}

/// The sidecar's stamp, or `None` when there's no sidecar.
pub fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(Stamp { modified_ms: modified.as_millis() as u64, len: meta.len() })
}

/// The position set in the sidecar of `photo`, as Lightroom Classic's Map module sets one:
/// `exif:GPSLatitude` and `exif:GPSLongitude`, as `DDD,MM,SSk` or `DDD,MM.mmk` (record 0010).
/// None when the photo gets no sidecar, or it isn't there, or holds no position. Read fresh each
/// time, as another app may set it.
pub fn position(photo: &Path) -> Option<Position> {
    if !has_sidecar(photo) {
        return None;
    }
    let meta = parse(&path_for(photo)).ok()??;
    let latitude = xmp_gps::exif_latitude_to_decimal(&meta.property(xmp_ns::EXIF, "GPSLatitude")?.value)?;
    let longitude = xmp_gps::exif_longitude_to_decimal(&meta.property(xmp_ns::EXIF, "GPSLongitude")?.value)?;
    Position::new(latitude, longitude)
}

/// What the sidecar at `path` says. A field it doesn't hold reads as none: no stars, unpicked,
/// no label, as Lightroom leaves out what isn't set.
pub fn read(path: &Path) -> Result<Rating, SidecarError> {
    let meta = parse(path)?.ok_or_else(|| format!("{} isn't there", path.display()))?;
    let text = |ns: &str, name: &str| meta.property(ns, name).map(|v| v.value.trim().to_lowercase());
    let xmp_rating: i32 = text(xmp_ns::XMP, "Rating").and_then(|r| r.parse::<f32>().ok()).map_or(0, |r| r.round() as i32);
    let pick = match (text(XMP_DM, "pick").as_deref(), text(XMP_DM, "good").as_deref()) {
        (Some("1"), _) => Pick::Picked,
        (Some("-1"), _) => Pick::Rejected,
        (Some(_), _) => Pick::Unpicked,
        (None, Some("true")) => Pick::Picked,
        (None, Some("false")) => Pick::Rejected,
        // Bridge marks a reject with a rating of -1.
        _ if xmp_rating < 0 => Pick::Rejected,
        _ => Pick::Unpicked,
    };
    let label = text(xmp_ns::XMP, "Label").and_then(|l| our_label(&l)).unwrap_or(Label::None);
    Ok(Rating { pick, stars: xmp_rating.clamp(0, 5) as u8, label })
}

/// Writes `rating` into the sidecar at `path`, keeping all else it holds, or makes one; and
/// returns its stamp after.
pub fn write(path: &Path, rating: Rating) -> Result<Stamp, SidecarError> {
    let mut meta = match parse(path)? {
        Some(meta) => meta,
        None => XmpMeta::new().map_err(|e| e.to_string())?,
    };
    let failed = |e: xmp_toolkit::XmpError| format!("can't set the rating in {}: {e}", path.display());

    meta.set_property(xmp_ns::XMP, "Rating", &XmpValue::new(rating.stars.min(5).to_string())).map_err(failed)?;
    // Lightroom Classic 13.2's flags: picked 1 and good, unpicked 0, rejected -1 and not good.
    let (pick, good) = match rating.pick {
        Pick::Picked => ("1", Some(true)),
        Pick::Unpicked => ("0", None),
        Pick::Rejected => ("-1", Some(false)),
    };
    meta.set_property(XMP_DM, "pick", &XmpValue::new(pick.to_string())).map_err(failed)?;
    match good {
        Some(good) => meta.set_property_bool(XMP_DM, "good", &XmpValue::new(good)).map_err(failed)?,
        None => meta.delete_property(XMP_DM, "good").map_err(failed)?,
    }
    match LABELS.iter().find(|(label, _)| *label == rating.label) {
        Some((_, name)) => meta.set_property(xmp_ns::XMP, "Label", &XmpValue::new(name.to_string())).map_err(failed)?,
        // A label PhotoMatch doesn't have, such as Purple, stays.
        None => {
            let theirs = meta.property(xmp_ns::XMP, "Label").map(|v| v.value);
            if theirs.is_some_and(|l| our_label(&l).is_some()) {
                meta.delete_property(xmp_ns::XMP, "Label").map_err(failed)?;
            }
        }
    }
    let now = XmpDateTime::current().map_err(failed)?;
    meta.set_property_date(xmp_ns::XMP, "MetadataDate", &XmpValue::new(now)).map_err(failed)?;

    let text = meta
        .to_string_with_options(ToStringOptions::default().omit_packet_wrapper().use_compact_format())
        .map_err(|e| format!("can't write {}: {e}", path.display()))?;
    write_durably(path, text.as_bytes())?;
    Ok(stamp(path).ok_or_else(|| format!("{} is gone after writing it", path.display()))?)
}

/// The sidecar at `path`, `None` when there's none, or why it can't be read. A sidecar must
/// hold the `x:xmpmeta` element: without it, the toolkit reads a damaged file as an empty one.
fn parse(path: &Path) -> Result<Option<XmpMeta>, SidecarError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(SidecarError::Disk(format!("can't read {}: {e}", path.display()))),
    };
    XmpMeta::from_str_with_options(&text, FromStrOptions::default().require_xmp_meta())
        .map(Some)
        .map_err(|e| SidecarError::Damaged(format!("can't read {}, so it's left as it is ({})", path.display(), e.debug_message)))
}

fn our_label(name: &str) -> Option<Label> {
    LABELS.iter().find(|(_, ours)| ours.eq_ignore_ascii_case(name.trim())).map(|(label, _)| *label)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sidecar as Lightroom Classic writes one, with edits of its own.
    const LIGHTROOM: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core 7.0-c000 1.000000, 0000/00/00-00:00:00        ">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:xmpDM="http://ns.adobe.com/xmp/1.0/DynamicMedia/"
    xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
   xmp:Rating="3" xmp:Label="Purple" xmpDM:pick="1" xmpDM:good="true"
   crs:Version="16.0" crs:Exposure2012="+0.35">
   <crs:ToneCurvePV2012><rdf:Seq><rdf:li>0, 0</rdf:li><rdf:li>255, 255</rdf:li></rdf:Seq></crs:ToneCurvePV2012>
   <dc:subject><rdf:Bag><rdf:li>Lisbon</rdf:li></rdf:Bag></dc:subject>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("photomatch-sidecar-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn raw_files_get_one_but_not_dngs_or_image_files() {
        assert!(has_sidecar(Path::new("/a/DSC01234.ARW")));
        assert!(has_sidecar(Path::new("/a/IMG_1.cr3")));
        assert!(!has_sidecar(Path::new("/a/L1000.DNG")));
        assert!(!has_sidecar(Path::new("/a/DSC01234.JPG")));
        assert!(!has_sidecar(Path::new("/a/IMG_1.HEIC")));
        assert_eq!(path_for(Path::new("/nowhere/DSC01234.ARW")), Path::new("/nowhere/DSC01234.xmp"));
    }

    #[test]
    fn a_new_sidecar_reads_back() {
        let dir = folder("new");
        let path = dir.join("DSC1.xmp");
        for rating in [
            Rating { pick: Pick::Picked, stars: 4, label: Label::Green },
            Rating { pick: Pick::Rejected, stars: 1, label: Label::Red },
            Rating { pick: Pick::Unpicked, stars: 0, label: Label::None },
        ] {
            let stamp = write(&path, rating).unwrap();
            assert_eq!(read(&path).unwrap(), rating);
            assert_eq!(Some(stamp), super::stamp(&path));
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("xmp:Label") && !text.contains("xmpDM:good"), "no label and no flag leave no trace:\n{text}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lightroom_s_own_fields_are_kept() {
        let dir = folder("lightroom");
        let path = dir.join("DSC2.xmp");
        std::fs::write(&path, LIGHTROOM).unwrap();
        assert_eq!(read(&path).unwrap(), Rating { pick: Pick::Picked, stars: 3, label: Label::None }, "Purple isn't one of ours");

        write(&path, Rating { pick: Pick::Rejected, stars: 5, label: Label::None }).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        for kept in ["crs:Exposure2012=\"+0.35\"", "crs:Version=\"16.0\"", "<rdf:li>255, 255</rdf:li>", "<rdf:li>Lisbon</rdf:li>", "xmp:Label=\"Purple\""] {
            assert!(text.contains(kept), "{kept} is gone:\n{text}");
        }
        assert!(text.contains("xmp:Rating=\"5\"") && text.contains("xmpDM:pick=\"-1\"") && text.contains("xmpDM:good=\"False\""), "{text}");
        assert!(text.contains("xmp:MetadataDate="), "{text}");

        write(&path, Rating { pick: Pick::Unpicked, stars: 5, label: Label::Blue }).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("xmp:Label=\"Blue\"") && !text.contains("xmpDM:good"), "{text}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A sidecar as another app might write it, with fields in a namespace of its own.
    const OTHER_APP: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:otherapp="http://example.com/otherapp/1.0/"
    otherapp:Edits="warmer">
   <otherapp:Tags>
    <rdf:Bag>
     <rdf:li>kept</rdf:li>
    </rdf:Bag>
   </otherapp:Tags>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

    /// Another app's own fields, in a namespace of its own, are kept as Lightroom's are: what the
    /// XMP specification asks of an app that changes a sidecar. (photo-library: its record 0008.)
    #[test]
    fn another_app_s_own_fields_are_kept() {
        let dir = folder("other-app");
        let path = dir.join("DSC3.xmp");
        std::fs::write(&path, OTHER_APP).unwrap();
        write(&path, Rating { pick: Pick::Picked, stars: 2, label: Label::Green }).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        for kept in ["http://example.com/otherapp/1.0/", "warmer", "<rdf:li>kept</rdf:li>"] {
            assert!(text.contains(kept), "{kept} is gone:\n{text}");
        }
        assert_eq!(read(&path).unwrap(), Rating { pick: Pick::Picked, stars: 2, label: Label::Green });
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_damaged_sidecar_is_left_as_it_is() {
        let dir = folder("damaged");
        let path = dir.join("DSC3.xmp");
        for damaged in ["", &LIGHTROOM[..LIGHTROOM.len() / 2], "rating=3", "\u{0}\u{1} not xml"] {
            std::fs::write(&path, damaged).unwrap();
            assert!(write(&path, Rating { stars: 2, ..Default::default() }).is_err(), "{damaged:?}");
            assert!(read(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), damaged);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn other_apps_flags_and_labels_read_as_ours() {
        let dir = folder("others");
        let path = dir.join("DSC4.xmp");
        let sidecar = |fields: &str| format!(r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:xmpDM="http://ns.adobe.com/xmp/1.0/DynamicMedia/" {fields}/></rdf:RDF></x:xmpmeta>"#);
        for (fields, expected) in [
            (r#"xmp:Rating="-1""#, Rating { pick: Pick::Rejected, ..Default::default() }),
            (r#"xmpDM:good="True" xmp:Label="yellow""#, Rating { pick: Pick::Picked, label: Label::Yellow, ..Default::default() }),
            (r#"xmpDM:good="False" xmp:Rating="2""#, Rating { pick: Pick::Rejected, stars: 2, ..Default::default() }),
            (r#"xmpDM:pick="0" xmpDM:good="true""#, Rating::default()),
            (r#"xmp:Rating="7""#, Rating { stars: 5, ..Default::default() }),
        ] {
            std::fs::write(&path, sidecar(fields)).unwrap();
            assert_eq!(read(&path).unwrap(), expected, "{fields}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A sidecar with a position, as Lightroom Classic's Map module writes one: `DDD,MM.mmk`.
    const POSITIONED: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:exif="http://ns.adobe.com/exif/1.0/"
    exif:GPSLatitude="37,48.4902N"
    exif:GPSLongitude="122,28.5432W"/>
 </rdf:RDF>
</x:xmpmeta>"#;

    /// A position set in a sidecar reads, and stays when a rating is written beside it (record 0010).
    #[test]
    fn a_position_set_in_a_sidecar_reads_and_stays_through_a_rating() {
        let dir = folder("position");
        let photo = dir.join("DSC5.ARW");
        std::fs::write(&photo, b"x").unwrap();
        assert_eq!(position(&photo), None, "no sidecar yet");
        std::fs::write(dir.join("DSC5.xmp"), POSITIONED).unwrap();
        let at = position(&photo).expect("a position");
        assert!((at.latitude - 37.80817).abs() < 1e-4 && (at.longitude + 122.47572).abs() < 1e-4, "{at:?}");
        write(&path_for(&photo), Rating { pick: Pick::Picked, stars: 3, label: Label::Red }).unwrap();
        assert_eq!(position(&photo), Some(at), "kept through a rating");
        std::fs::write(dir.join("L5.xmp"), POSITIONED).unwrap();
        assert_eq!(position(&dir.join("L5.DNG")), None, "a DNG gets no sidecar");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
