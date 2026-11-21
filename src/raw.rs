//! Reading RAW files with LibRaw, behind the app's photo interface (record 0007).

use std::path::Path;

use image::ImageFormat;

use crate::photo::{has_extension, Orientation, Photo, PhotoInfo, PhotoReader, Preview, Want};
use crate::problem::Problem;
use crate::thumbs;

/// The JPEG quality of a photo developed from its RAW data, as for the large view's.
const DEVELOPED_QUALITY: u8 = 90;

/// The RAW extensions listed: every still photo's that LibRaw reads and has the colours for, so
/// that the app can show it (record 0009). LibRaw knows a file by its contents, not its name, so a
/// file named so that it can't read shows as one it can't. Left out: video RAW (`.ari`, `.cine`,
/// `.cin`, `.r3d`); `.hdr`, which Radiance HDR images share; GoPro's `.gpr`, which LibRaw reads
/// only with GoPro's SDK, and Sigma's `.x3f`, only with code it leaves off by default; and the
/// oldest cameras' formats, for which it has no colour matrix: `.mdc`, `.dc2`, `.k25`, `.kc2`,
/// `.pxn` and `.bay`.
const EXTENSIONS: &[&str] = &[
    // PhotoMatch's first ten (its plan steps 2.4 and later).
    "arw", "cr2", "cr3", "nef", "raf", "dng", "rw2", "rwl", "orf", "pef",
    // Each read and developed from a sample from raw.pixls.us (record 0009).
    "3fr", "arq", "crw", "dcr", "erf", "fff", "iiq", "kdc", "mos", "mrw", "nrw", "ori", "raw", "sr2", "srf", "srw", "sti",
    // No sample to try; LibRaw names the cameras that write them, and has their colours (record 0009).
    "cap", "cs1", "ia", "mef", "qtk",
];

pub fn is_raw(path: &Path) -> bool {
    has_extension(path, EXTENSIONS)
}

pub struct LibRawReader;

impl PhotoReader for LibRawReader {
    fn read(&self, path: &Path, want: Want) -> Result<Photo, Problem> {
        read(path, want, false)
    }
}

/// Reads a RAW file and develops it from its RAW data, even when it holds a JPEG preview, as
/// for one that doesn't, at half size or at `full` size: for `develop_once`, to try the app's
/// way of developing on any sample.
pub fn read_developed(path: &Path, full: bool) -> Result<Photo, Problem> {
    read(path, if full { Want::Full } else { Want::Largest }, true)
}

/// Opens the file by its path through LibRaw's own reader, which reads only what it's asked for
/// (record 0016), on Windows through its wide-character call (record 0031).
fn open(path: &Path) -> Result<rsraw::RawImage, Problem> {
    rsraw::RawImage::open_file(path).map_err(|e| opening_problem(path, e))
}

/// What it means when LibRaw can't open a file (plan step 2.35). LibRaw says only `Io` for a file
/// that isn't there or can't be read, so the file itself is asked first. Then a file it doesn't
/// recognise isn't a photo it reads, which may be a damaged one or a camera it doesn't know
/// (unchecked whether it can tell them apart), and one whose data runs out or is wrong is
/// damaged.
fn opening_problem(path: &Path, error: rsraw::Error) -> Problem {
    if let Err(e) = std::fs::File::open(path) {
        return Problem::reading(path, e);
    }
    let cause = format!("LibRaw can't open {}: {error}", path.display());
    match error {
        rsraw::Error::FileUnsupported => Problem::NotAPhoto { cause },
        rsraw::Error::Io | rsraw::Error::Data | rsraw::Error::InputClosed => Problem::Damaged { cause },
        _ => Problem::Internal { cause },
    }
}

/// Takes out the preview `want` asks for, among the JPEGs LibRaw lists: the smallest whose long
/// side is long enough, or else the largest. One listed as a JPEG that turns out otherwise once
/// taken out, as a Canon CR3's HEIF does, is passed over for the next.
fn take_preview(raw: &mut rsraw::RawImage, want: Want) -> Option<Preview> {
    let listed = raw.list_thumbs();
    // Each candidate's place in LibRaw's list, and its size.
    let mut candidates: Vec<(usize, u32, u32)> = (0..listed.len()).filter(|&i| listed[i].jpeg).map(|i| (i, listed[i].width, listed[i].height)).collect();
    // When the largest is wanted, one listed as 0 × 0 may be it, as the Canon R6 III's full-size
    // preview is: taken out, its own header says its size (plan step 2.26).
    let mut taken = Vec::new();
    if matches!(want, Want::Largest | Want::Full) {
        for candidate in candidates.iter_mut().filter(|c| c.1 == 0 || c.2 == 0) {
            if let Some(preview) = take_jpeg(raw, candidate.0) {
                (candidate.1, candidate.2) = (preview.width, preview.height);
                taken.push((candidate.0, preview));
            }
        }
    }
    candidates.sort_by_key(|&(_, width, height)| width as u64 * height as u64);
    while !candidates.is_empty() {
        let at = match want {
            Want::AtLeast(long_side) => candidates.iter().position(|&(_, width, height)| width.max(height) >= long_side).unwrap_or(candidates.len() - 1),
            Want::Largest | Want::Full => candidates.len() - 1,
            // The smallest of a size LibRaw knows: one listed as 0 × 0 may be the largest.
            Want::Smallest => candidates.iter().position(|&(_, width, height)| width > 0 && height > 0).unwrap_or(0),
        };
        let (index, ..) = candidates.remove(at);
        let already = taken.iter().position(|(i, _)| *i == index).map(|at| taken.swap_remove(at).1);
        if let Some(preview) = already.or_else(|| take_jpeg(raw, index)) {
            return Some(preview);
        }
    }
    None
}

/// The preview at `index` of LibRaw's list, if it's a JPEG, with its size from its own header
/// where LibRaw doesn't know it.
fn take_jpeg(raw: &mut rsraw::RawImage, index: usize) -> Option<Preview> {
    let thumb = raw.extract_thumb(index).ok().filter(|t| t.format == rsraw::ThumbFormat::Jpeg)?;
    let (width, height) = if thumb.width > 0 && thumb.height > 0 { (thumb.width, thumb.height) } else { jpeg_size(&thumb.data)? };
    Some(Preview { width, height, format: ImageFormat::Jpeg, bytes: thumb.data, tagged_orientation: false, made: false })
}

fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
    decoder.read_info().ok()?;
    decoder.info().map(|info| (info.width as u32, info.height as u32))
}

fn read(path: &Path, want: Want, always_develop: bool) -> Result<Photo, Problem> {
    let mut raw = open(path)?;
    // The largest preview's size, as listed, before one is taken out.
    let largest = raw.list_thumbs().into_iter().filter(|t| t.jpeg).max_by_key(|t| t.width as u64 * t.height as u64);
    // Developing anyway still takes out the preview at full size, as the tones to match.
    let mut previews: Vec<Preview> = if always_develop && !matches!(want, Want::Full) { None } else { take_preview(&mut raw, want) }.into_iter().collect();

    // The details and the orientation before developing, which sets LibRaw's orientation to the
    // one asked for (`raw2image.cpp`, lines 28 and 29). LibRaw's own data holds the orientation,
    // which `rsraw` doesn't wrap.
    let data: &rsraw_sys::libraw_data_t = raw.as_ref();
    let orientation = match data.sizes.flip {
        3 => Orientation::Rotate180,
        5 => Orientation::Rotate270,
        6 => Orientation::Rotate90,
        _ => Orientation::Normal,
    };
    let lens = raw.lens_info();
    let (width, height) = if orientation.is_sideways() { (raw.height(), raw.width()) } else { (raw.width(), raw.height()) };
    let mut info = PhotoInfo {
        make: raw.make().trim().to_string(),
        model: raw.model().trim().to_string(),
        lens: lens.lens_name.trim().to_string(),
        // LibRaw's 0 is a photo without its time, as a Leaf back's, not 1 January 1970 (record 0009).
        captured_at: raw.datetime().filter(|_| data.other.timestamp > 0).map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
        iso: raw.iso_speed(),
        shutter_seconds: raw.shutter(),
        aperture: raw.aperture(),
        focal_length_mm: raw.focal_len(),
        width,
        height,
        preview_width: 0,
        preview_height: 0,
        thumb_width: 0,
        thumb_height: 0,
        no_preview: None,
    };

    // No JPEG preview to show, as with the Canon R5 Mark II's HEIF ones (finding 6 in
    // docs/photo-browser.md): the photo is developed from its RAW data instead, if LibRaw
    // knows the camera. If not, its colours come out wrong, so it isn't shown at all. Either
    // way, or if developing fails, the photo keeps its details.
    //
    // At full resolution, a preview at least nine tenths of the photo's own size is the photo;
    // a smaller one, as older Sony bodies embed, gives way to the RAW data developed at full
    // size (record 0015). If that fails, the photo keeps its preview.
    let camera = format!("{} {}", raw.normalized_make(), raw.normalized_model());
    let full = want == Want::Full;
    let full_size = |p: &Preview| p.width.max(p.height) as u64 * 10 >= width.max(height) as u64 * 9;
    let mut no_preview = None;
    let small = previews.last().is_some_and(|p| too_small(p.width, p.height, want));
    if previews.is_empty() || always_develop || (full && !previews.iter().any(full_size)) || small {
        if knows_colours(&raw) {
            // A photo developed at full size takes its tones from the camera's own preview, so
            // zooming in shows more detail, not another look (plan step 2.26).
            let reference = previews.last().filter(|_| full).and_then(|p| thumbs::decode(p).ok());
            match develop(&mut raw, !full, reference.as_ref()) {
                Ok(developed) => previews = vec![developed],
                Err(error) if previews.is_empty() => no_preview = Some(Problem::DevelopFailed { cause: format!("no preview PhotoMatch can show, and developing it failed: {error}") }),
                Err(_) => {}
            }
        } else if previews.is_empty() {
            no_preview = Some(Problem::NoPreviewCamera { camera: camera.trim().to_string() });
        }
    }
    // The largest preview's size: the one taken out, or the largest listed if that's larger.
    let taken = previews.last().map_or((0, 0), |p| (p.width, p.height));
    (info.preview_width, info.preview_height) = match largest {
        Some(listed) if !always_develop && listed.width as u64 * listed.height as u64 > taken.0 as u64 * taken.1 as u64 => (listed.width, listed.height),
        _ => taken,
    };

    Ok(Photo { previews, no_preview, orientation, info })
}

/// The long side under which a preview is too small for the large view, if it's under half the
/// size asked for too: older cameras embed 640 × 480, a Leaf back 332 × 244, while the smallest
/// preview of a newer camera tried, older Sony bodies', is 1616 × 1080 (record 0009).
const SMALL_PREVIEW: u32 = 1280;

/// Whether a preview is too small for what's asked, so that the RAW data developed at half size
/// takes its place, sharper (record 0015; record 0009). Never at full resolution, which has its
/// own rule, and never for a newer camera's preview, whichever the screen.
fn too_small(width: u32, height: u32, want: Want) -> bool {
    let long = width.max(height);
    matches!(want, Want::AtLeast(asked) if long * 2 < asked && long < SMALL_PREVIEW)
}

/// Whether LibRaw has the camera's colours, to develop its photos: its colour matrix, which
/// LibRaw fills from its own table for a camera it knows, and a DNG brings with it. Without one,
/// as the Canon R5 Mark II was to LibRaw 0.21.3, colours come out wrong (finding 6). Matching
/// the camera's name against LibRaw's list, as before, missed the RX100 VII, the S5 II and the
/// OM-1, which the list names differently (plan step 2.26).
fn knows_colours(raw: &rsraw::RawImage) -> bool {
    let colours: &rsraw_sys::libraw_data_t = raw.as_ref();
    colours.color.cam_xyz[0].iter().any(|&v| v != 0.0)
}

/// The photo developed from its RAW data by LibRaw, as a JPEG: at half size, which is faster and
/// large enough for the screen, or at full size, for zooming in (plan step 2.26). It's left
/// unturned, like an embedded preview, so the app turns it once, by the photo's orientation.
fn develop(raw: &mut rsraw::RawImage, half: bool, reference: Option<&image::DynamicImage>) -> Result<Preview, String> {
    raw.set_half_size(half);
    raw.set_use_camera_wb(true);
    raw.as_mut().params.user_flip = 0;
    // LibRaw's own demosaic, AHD: PPG was 30% faster, and the owner found it less sharp at 100%
    // (plan step 2.51, records 0032 and 0033).
    raw.unpack().map_err(|e| e.to_string())?;
    let developed = raw.process::<{ rsraw::BIT_DEPTH_8 }>().map_err(|e| e.to_string())?;
    let (width, height) = (developed.width(), developed.height());
    let mut rgb = image::RgbImage::from_raw(width, height, developed.to_vec()).ok_or("LibRaw's image isn't 8-bit RGB")?;
    if let Some(reference) = reference {
        match_tones(&mut rgb, &reference.to_rgb8());
    }
    let jpeg = thumbs::encode(&image::DynamicImage::ImageRgb8(rgb), DEVELOPED_QUALITY)?;
    Ok(Preview { width, height, format: ImageFormat::Jpeg, bytes: jpeg, tagged_orientation: false, made: true })
}

/// Gives `developed` the tones of `reference`, the same photo as the camera rendered it, channel
/// by channel: each level of the developed photo takes the level at the same place in the
/// reference's distribution of levels, compared at the reference's size. The camera's tone curve
/// and picture style aren't LibRaw's to know; this carries them over.
fn match_tones(developed: &mut image::RgbImage, reference: &image::RgbImage) {
    let small = image::imageops::resize(developed, reference.width(), reference.height(), image::imageops::FilterType::Triangle);
    let mut maps = [[0u8; 256]; 3];
    for (channel, map) in maps.iter_mut().enumerate() {
        let cumulative = |image: &image::RgbImage| {
            let mut counts = [0u64; 256];
            for pixel in image.pixels() {
                counts[pixel[channel] as usize] += 1;
            }
            let total = counts.iter().sum::<u64>().max(1) as f64;
            let mut running = 0u64;
            counts.map(|n| {
                running += n;
                running as f64 / total
            })
        };
        let (from, to) = (cumulative(&small), cumulative(reference));
        let mut level = 0;
        for (value, share) in from.iter().enumerate() {
            while level < 255 && to[level] < *share {
                level += 1;
            }
            map[value] = level as u8;
        }
    }
    for pixel in developed.pixels_mut() {
        for channel in 0..3 {
            pixel[channel] = maps[channel][pixel[channel] as usize];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A preview under half the size asked for and under 1280 pixels long gives way to the RAW
    /// data developed; a newer camera's never does, even on a 4K screen (record 0009).
    #[test]
    fn only_an_older_camera_s_small_preview_gives_way_to_the_raw_data() {
        assert!(too_small(640, 480, Want::AtLeast(2560)), "640 × 480 on a screen");
        assert!(too_small(332, 244, Want::AtLeast(800)), "332 × 244 for a thumbnail");
        assert!(!too_small(640, 480, Want::AtLeast(800)), "640 × 480 for a thumbnail");
        assert!(!too_small(1616, 1080, Want::AtLeast(3840)), "1616 × 1080 on a 4K screen");
        assert!(!too_small(332, 244, Want::Full) && !too_small(332, 244, Want::Largest) && !too_small(332, 244, Want::Smallest));
    }

    /// Still photos' RAW extensions are listed, in either case; video RAW, Radiance's HDR images
    /// and the formats the app can't show aren't (record 0009).
    #[test]
    fn still_photo_raw_extensions_are_listed_and_video_raw_isn_t() {
        for name in ["DSC1.ARW", "DSC1.SR2", "dsc1.srf", "IMG_1.CRW", "P1.RAW", "H.3FR", "CF.IIQ", "S.STI", "Q.QTK", "D.ARQ"] {
            assert!(is_raw(Path::new(name)), "{name} isn't listed");
        }
        for name in ["clip.ari", "clip.R3D", "clip.cine", "clip.cin", "sky.hdr", "G.GPR", "B.X3F", "M.MDC", "DSC1.JPG", "DSC1.xmp", "DSC1"] {
            assert!(!is_raw(Path::new(name)), "{name} is listed");
        }
    }
}
