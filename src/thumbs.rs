//! Thumbnails made from each photo's preview, cached on disk (record 0009): a RAW file's
//! embedded JPEG, or an image file itself.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Instant;

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat};
use serde::Serialize;

use crate::files::write_atomically;
use crate::photo::{Orientation, Photo, PhotoInfo, PhotoReader, Preview, Want, DETAILS_VERSION};
use crate::problem::Problem;

/// The long side of a thumbnail, in pixels: sharp in rows up to 267 points high for a 3:2 photo
/// on a 2x screen, viewing quality before speed (record 0015).
pub const THUMB_SIZE: u32 = 800;
const THUMB_QUALITY: u8 = 85;
const PREVIEW_QUALITY: u8 = 90;
/// Bump to make every cached thumbnail and its details again after a change in how
/// they're made. 2: the details give the upright size. 3: and the thumbnail's size. 4: 800
/// pixels, shrunk with Lanczos3 (plan step 2.25).
const CACHE_VERSION: u32 = 4;
/// Bump to make every full-size photo again, and none of the rest, after a change in how they're
/// developed. 2: without LibRaw's brightening, where the camera's tones are matched (record 0013).
const FULL_VERSION: u32 = 2;

pub struct ThumbCache {
    dir: PathBuf,
    stats: Counters,
    /// The photos being read now, by cache key, so that the grid and the details read in
    /// the background read each file once.
    reading: Mutex<HashSet<String>>,
    read_done: Condvar,
    /// Why a photo couldn't be read, by cache key, so that it isn't read again until the
    /// file changes or the app restarts.
    failed: Mutex<HashMap<String, Problem>>,
}

/// A turn to read one photo: others asking for it wait until it's dropped.
struct Turn<'a> {
    cache: &'a ThumbCache,
    key: String,
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        if let Ok(mut reading) = self.cache.reading.lock() {
            reading.remove(&self.key);
        }
        self.cache.read_done.notify_all();
    }
}

#[derive(Default)]
struct Counters {
    made: AtomicU64,
    from_cache: AtomicU64,
    make_ms_total: AtomicU64,
    make_ms_max: AtomicU64,
    bytes_written: AtomicU64,
    /// Full-size photos made and kept, ahead of a zoom or for one (plan step 2.51).
    fulls_made: AtomicU64,
    /// Full-size photos `prepare_full` took on, which a zoom hadn't made already (plan step 2.51).
    prepared: AtomicU64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbStats {
    pub made: u64,
    pub from_cache: u64,
    pub make_ms_average: u64,
    pub make_ms_max: u64,
    pub bytes_written: u64,
    pub fulls_made: u64,
    pub prepared: u64,
}

impl ThumbCache {
    pub fn new(dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir, stats: Counters::default(), reading: Mutex::default(), read_done: Condvar::new(), failed: Mutex::default() })
    }

    /// Waits while another thread reads the photo with this key, then takes the turn.
    fn turn(&self, key: String) -> Result<Turn<'_>, Problem> {
        let reading = self.reading.lock().map_err(|e| e.to_string())?;
        let mut reading = self.read_done.wait_while(reading, |r| r.contains(&key)).map_err(|e| e.to_string())?;
        reading.insert(key.clone());
        Ok(Turn { cache: self, key })
    }

    /// The thumbnail of `path`, from the cache or made now.
    pub fn thumbnail(&self, reader: &dyn PhotoReader, path: &Path) -> Result<Vec<u8>, Problem> {
        let key = cache_key(path)?;
        let _turn = self.turn(key.clone())?;
        if let Ok(bytes) = std::fs::read(self.dir.join(format!("{key}.jpg"))) {
            self.stats.from_cache.fetch_add(1, Ordering::Relaxed);
            return Ok(bytes);
        }
        self.make(reader, path, &key)?.0
    }

    /// The photo's details, from the cache or read now. Reading them makes the thumbnail
    /// too, since reading the file is most of the work.
    pub fn info(&self, reader: &dyn PhotoReader, path: &Path) -> Result<PhotoInfo, Problem> {
        let key = cache_key(path)?;
        let _turn = self.turn(key.clone())?;
        if let Some(cached) = std::fs::read(self.dir.join(format!("{key}.json"))).ok().and_then(|b| serde_json::from_slice::<PhotoInfo>(&b).ok()) {
            if cached.details_version >= DETAILS_VERSION {
                return Ok(cached);
            }
            // Cached before what the details hold grew: they're read again, without a preview,
            // and the sizes and the thumbnail kept (record 0010). If the file can't be read now,
            // the details stay as they were.
            let Ok(photo) = reader.read(path, Want::Details) else { return Ok(cached) };
            let info = PhotoInfo {
                width: cached.width,
                height: cached.height,
                preview_width: cached.preview_width,
                preview_height: cached.preview_height,
                thumb_width: cached.thumb_width,
                thumb_height: cached.thumb_height,
                no_preview: cached.no_preview,
                ..photo.info
            };
            self.keep(&format!("{key}.json"), &serde_json::to_vec(&info).map_err(|e| e.to_string())?);
            return Ok(info);
        }
        Ok(self.make(reader, path, &key)?.1)
    }

    /// Reads the photo, and caches its details and its thumbnail. A photo whose details read
    /// but which can't be shown keeps its details, with why, and has no thumbnail. Why a file
    /// can't be read, or shown, is remembered until it changes or the app restarts.
    fn make(&self, reader: &dyn PhotoReader, path: &Path, key: &str) -> Result<(Result<Vec<u8>, Problem>, PhotoInfo), Problem> {
        let remembered = self.failed.lock().map_err(|e| e.to_string())?.get(key).cloned();
        if let Some(error) = remembered {
            return Err(error);
        }
        let remember = |error: &Problem| {
            if let Ok(mut failed) = self.failed.lock() {
                failed.insert(key.to_string(), error.clone());
            }
        };
        let started = Instant::now();
        let photo = reader.read(path, Want::AtLeast(THUMB_SIZE)).inspect_err(remember)?;
        let thumb = make_thumbnail(&photo);
        let info = match &thumb {
            Ok(thumb) => PhotoInfo { thumb_width: thumb.width, thumb_height: thumb.height, ..photo.info },
            Err(why) => PhotoInfo { no_preview: Some(why.clone()), ..photo.info },
        };
        self.keep(&format!("{key}.json"), &serde_json::to_vec(&info).map_err(|e| e.to_string())?);
        let thumb = match thumb {
            Ok(thumb) => thumb,
            Err(why) => {
                remember(&why);
                return Ok((Err(why), info));
            }
        };
        self.keep(&format!("{key}.jpg"), &thumb.jpeg);

        let ms = started.elapsed().as_millis() as u64;
        self.stats.made.fetch_add(1, Ordering::Relaxed);
        self.stats.make_ms_total.fetch_add(ms, Ordering::Relaxed);
        self.stats.make_ms_max.fetch_max(ms, Ordering::Relaxed);
        self.stats.bytes_written.fetch_add(thumb.jpeg.len() as u64, Ordering::Relaxed);
        Ok((Ok(thumb.jpeg), info))
    }

    /// The large view's image, upright, `long_side` pixels on its long side at most: the size of
    /// the screen (plan step 2.26). Made once and kept. A preview already that small and upright
    /// is served as the file stores it.
    pub fn screen(&self, reader: &dyn PhotoReader, path: &Path, long_side: u32) -> Result<(Vec<u8>, &'static str), Problem> {
        let name = format!("{}-screen-{long_side}.jpg", cache_key(path)?);
        let _turn = self.turn(name.clone())?;
        if let Ok(bytes) = std::fs::read(self.dir.join(&name)) {
            return Ok((bytes, "image/jpeg"));
        }
        let photo = reader.read(path, Want::AtLeast(long_side))?;
        let preview = photo.preview_for(long_side)?;
        if !preview.made && preview.width.max(preview.height) <= long_side && upright_as_stored(photo.orientation, preview) {
            if let Some(media_type) = web_media_type(preview.format) {
                return Ok((preview.bytes.clone(), media_type));
            }
        }
        let jpeg = encode(&upright(shrink(decode_for(preview, long_side)?, long_side), photo.orientation), PREVIEW_QUALITY)?;
        self.keep(&name, &jpeg);
        Ok((jpeg, "image/jpeg"))
    }

    /// The photo at full resolution, upright, for zooming in: the camera's full-size preview, or
    /// else the RAW data developed at full size, in the camera's tones (plan step 2.26). Kept
    /// once made; a full-size preview that needs nothing done is served as the file stores it.
    pub fn full(&self, reader: &dyn PhotoReader, path: &Path) -> Result<(Vec<u8>, &'static str), Problem> {
        let name = full_name(path)?;
        let _turn = self.turn(name.clone())?;
        if let Ok(bytes) = std::fs::read(self.dir.join(&name)) {
            return Ok((bytes, "image/jpeg"));
        }
        let photo = reader.read(path, Want::Full)?;
        let orientation = photo.orientation;
        let preview = photo.into_largest_preview()?;
        if !preview.made && upright_as_stored(orientation, &preview) {
            if let Some(media_type) = web_media_type(preview.format) {
                return Ok((preview.bytes, media_type));
            }
        }
        let jpeg = if orientation == Orientation::Normal && preview.format == ImageFormat::Jpeg {
            preview.bytes
        } else {
            encode(&upright(decode(&preview)?, orientation), PREVIEW_QUALITY)?
        };
        self.keep(&name, &jpeg);
        self.stats.fulls_made.fetch_add(1, Ordering::Relaxed);
        trim_full(&self.dir, FULL_CACHE_BYTES);
        Ok((jpeg, "image/jpeg"))
    }

    /// Makes the photo's full resolution ahead of a zoom, when the large view has stayed on it
    /// (plan step 2.51, record 0034): developed and kept, so that the zoom reads it from the cache.
    /// Nothing is read for a photo already kept.
    pub fn prepare_full(&self, reader: &dyn PhotoReader, path: &Path) -> Result<(), Problem> {
        if self.dir.join(full_name(path)?).exists() {
            return Ok(());
        }
        self.stats.prepared.fetch_add(1, Ordering::Relaxed);
        self.full(reader, path).map(|_| ())
    }

    /// Where the photo's highlights clip and its shadows sink, as a PNG the size of the image it
    /// goes over, `shown`, upright: red, blue and clear (the library's plan step 2.8, record 0014).
    /// From a RAW file's sensor data, never its preview; from an image file's own pixels. Made
    /// once and kept.
    pub fn clipping(&self, reader: &dyn PhotoReader, path: &Path, shown: (u32, u32)) -> Result<Vec<u8>, Problem> {
        let name = clipping_name(path, shown)?;
        let _turn = self.turn(name.clone())?;
        if let Ok(bytes) = std::fs::read(self.dir.join(&name)) {
            return Ok(bytes);
        }
        let marks = if crate::raw::is_raw(path) {
            crate::clipping::of_raw(path, shown)?
        } else {
            let photo = reader.read(path, Want::Largest)?;
            let orientation = photo.orientation;
            let image = upright(decode(&photo.into_largest_preview()?)?, orientation);
            crate::clipping::of_image(&image, shown)
        };
        let png = marks.png()?;
        self.keep(&name, &png);
        Ok(png)
    }

    /// The clipping marks of `path` at `shown` if they're made, without making them.
    pub fn cached_clipping(&self, path: &Path, shown: (u32, u32)) -> Option<Vec<u8>> {
        std::fs::read(self.dir.join(clipping_name(path, shown).ok()?)).ok()
    }

    /// The thumbnail of `path` if it's made, without waiting for a read under way or making it.
    pub fn cached_thumbnail(&self, path: &Path) -> Option<Vec<u8>> {
        std::fs::read(self.dir.join(format!("{}.jpg", cache_key(path).ok()?))).ok()
    }

    /// Keeps a file in the cache. A cache that can't be written to, full or deleted while the
    /// app runs, costs the time of reading the photo again later, never the photo itself: the
    /// folder is made again once, and if it still fails, the file isn't kept.
    /// The key a photo's cached files are kept under, from its path, size and time of change:
    /// taken before the photo moves, for `moved` (record 0016).
    pub fn key_of(path: &Path) -> Option<String> {
        cache_key(path).ok()
    }

    /// Photos moved, or copied, each from the key its files were kept under before to its new
    /// path: their thumbnails and details renamed to its new key, or for a copy, those two copied,
    /// so that they aren't made again. The cache is listed once for them all. A file that can't be
    /// renamed is made again when it's next asked for.
    ///
    /// Where a file's new name is another's old one, as for two photos of one size and time that
    /// swap names, every file goes through a hidden name first (record 0017).
    pub fn moved(&self, photos: &[(String, PathBuf)], copy: bool) {
        let renamed: HashMap<&str, String> = photos.iter().filter_map(|(key, to)| Some((key.as_str(), cache_key(to).ok()?))).collect();
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return };
        let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(new) = name.get(..32).and_then(|key| renamed.get(key)) else { continue };
            let rest = &name[32..];
            if !copy || rest == ".jpg" || rest == ".json" {
                files.push((entry.path(), self.dir.join(format!("{new}{rest}"))));
            }
        }
        let sources: HashSet<&PathBuf> = files.iter().map(|(from, _)| from).collect();
        let crossing = !copy && files.iter().any(|(_, to)| sources.contains(to));
        let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
        for (n, (from, to)) in files.iter().enumerate() {
            if crossing {
                let hidden = self.dir.join(format!(".moving-{}-{n}", std::process::id()));
                if std::fs::rename(from, &hidden).is_ok() {
                    moves.push((hidden, to.clone()));
                }
            } else {
                moves.push((from.clone(), to.clone()));
            }
        }
        for (from, to) in moves {
            let kept = if copy { std::fs::copy(&from, &to).map(|_| ()) } else { std::fs::rename(&from, &to) };
            if let Err(error) = kept {
                eprintln!("photomatch: the thumbnail cache: can't keep {} as {}: {error}", from.display(), to.display());
            }
        }
    }

    fn keep(&self, name: &str, bytes: &[u8]) {
        let path = self.dir.join(name);
        let kept = write_atomically(&path, bytes).or_else(|_| {
            std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
            write_atomically(&path, bytes)
        });
        if let Err(error) = kept {
            eprintln!("photomatch: the thumbnail cache: {error}");
        }
    }

    pub fn stats(&self) -> ThumbStats {
        let made = self.stats.made.load(Ordering::Relaxed);
        ThumbStats {
            made,
            from_cache: self.stats.from_cache.load(Ordering::Relaxed),
            make_ms_average: self.stats.make_ms_total.load(Ordering::Relaxed).checked_div(made).unwrap_or(0),
            make_ms_max: self.stats.make_ms_max.load(Ordering::Relaxed),
            bytes_written: self.stats.bytes_written.load(Ordering::Relaxed),
            fulls_made: self.stats.fulls_made.load(Ordering::Relaxed),
            prepared: self.stats.prepared.load(Ordering::Relaxed),
        }
    }
}

pub struct Thumbnail {
    pub jpeg: Vec<u8>,
    /// Upright, as shown.
    pub width: u32,
    pub height: u32,
}

/// The thumbnail of a photo already read, upright, as JPEG. Made from the smallest
/// preview that's large enough, since decoding a full-size one costs ten times more.
pub fn make_thumbnail(photo: &Photo) -> Result<Thumbnail, Problem> {
    let image = upright(shrink(decode_for(photo.preview_for(THUMB_SIZE)?, THUMB_SIZE)?, THUMB_SIZE), photo.orientation);
    Ok(Thumbnail { jpeg: encode(&image, THUMB_QUALITY)?, width: image.width(), height: image.height() })
}

/// The smallest preview a RAW file embeds, 160×120 on Sony bodies, upright, as JPEG: shown while
/// the thumbnail is made, so a folder seen for the first time fills at once (plan step 2.25).
pub fn quick_preview(photo: &Photo) -> Result<Vec<u8>, Problem> {
    let smallest = photo.previews.first().ok_or_else(|| photo.no_preview.clone().unwrap_or_else(|| "no preview".into()))?;
    // Bodies that embed nothing small, such as the Ricoh GR IIIx or the OM-1, would take 35 to
    // 230 ms: no quicker than the thumbnail, so their tiles wait for it.
    if smallest.width.max(smallest.height) > QUICK_MAX {
        return Err("no small preview".into());
    }
    Ok(encode(&upright(decode(smallest)?, photo.orientation), THUMB_QUALITY)?)
}

/// The largest preview a quick preview is made from: a few milliseconds to decode.
const QUICK_MAX: u32 = 1024;

/// Decodes a preview to be shown `long_side` pixels on its long side. A JPEG four times as
/// large or more is decoded at a fraction of its size, from ½ to ⅛, still at least
/// `long_side`: about twice as fast for a full-size preview (review for 0.2.0). Others are
/// decoded whole, by the `image` crate's decoder, the faster of the two at full size.
pub(crate) fn decode_for(preview: &Preview, long_side: u32) -> Result<DynamicImage, Problem> {
    if preview.format == ImageFormat::Jpeg && preview.width.max(preview.height) >= 4 * long_side {
        if let Some(image) = decode_scaled(&preview.bytes, long_side) {
            return Ok(image);
        }
    }
    decode(preview)
}

fn decode_scaled(bytes: &[u8], long_side: u32) -> Option<DynamicImage> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder.read_info().ok()?;
    let info = decoder.info()?;
    let (width, height) = (info.width as u32, info.height as u32);
    // The decoder picks the smallest scale that's still at least the size asked for.
    let at_least = |side: u32| (side * long_side).div_ceil(width.max(height)).clamp(1, u16::MAX as u32) as u16;
    let (width, height) = decoder.scale(at_least(width), at_least(height)).ok()?;
    let pixels = decoder.decode().ok()?;
    match decoder.info()?.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => image::RgbImage::from_raw(width as u32, height as u32, pixels).map(DynamicImage::ImageRgb8),
        jpeg_decoder::PixelFormat::L8 => image::GrayImage::from_raw(width as u32, height as u32, pixels).map(DynamicImage::ImageLuma8),
        _ => None,
    }
}

/// Shrinks an image to `long_side` on its long side with Lanczos3, a little crisper than
/// averaging, for 1 to 6 ms more (plan step 2.25). An image already that small stays as it is.
pub(crate) fn shrink(image: DynamicImage, long_side: u32) -> DynamicImage {
    let (width, height) = (image.width() as u64, image.height() as u64);
    if width.max(height) <= long_side as u64 {
        return image;
    }
    let other = |side: u64, long: u64| ((side * long_side as u64 + long / 2) / long).max(1) as u32;
    let (to_width, to_height) = if width >= height { (long_side, other(height, width)) } else { (other(width, height), long_side) };
    image.resize_exact(to_width, to_height, image::imageops::FilterType::Lanczos3)
}

/// Whether a web view shows the preview upright as it's stored: it needs no turn, or it's a
/// JPEG whose own EXIF says how to turn it, which web views follow.
fn upright_as_stored(orientation: Orientation, preview: &Preview) -> bool {
    orientation == Orientation::Normal || (preview.tagged_orientation && preview.format == ImageFormat::Jpeg)
}

/// The formats both OSes' web views show: WebKit on macOS and WebView2 on Windows. WebView2
/// doesn't show TIFF (unchecked), and BMP is too large to serve as stored.
fn web_media_type(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Png => Some("image/png"),
        ImageFormat::WebP => Some("image/webp"),
        ImageFormat::Gif => Some("image/gif"),
        _ => None,
    }
}

/// Identifies a file by its path, size and modification time. The store (step 1.3)
/// decides how the app identifies a photo for good.
fn cache_key(path: &Path) -> Result<String, Problem> {
    let meta = std::fs::metadata(path).map_err(|e| Problem::reading(path, e))?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let mut hasher = blake3::Hasher::new();
    hasher.update(path.as_os_str().as_encoded_bytes());
    hasher.update(&meta.len().to_le_bytes());
    hasher.update(&modified.to_le_bytes());
    hasher.update(&CACHE_VERSION.to_le_bytes());
    Ok(hasher.finalize().to_hex()[..32].to_string())
}

/// The version of the clipping marks, in the names they're kept under: a change in how they're
/// made leaves those made before unread.
const CLIPPING_VERSION: u32 = 1;

fn clipping_name(path: &Path, shown: (u32, u32)) -> Result<String, Problem> {
    Ok(format!("{}-clip-v{CLIPPING_VERSION}-{}x{}.png", cache_key(path)?, shown.0, shown.1))
}

/// The name a photo's full size is kept under. Ending in `-full.jpg`, as `trim_full` looks for, so
/// that those of an older `FULL_VERSION` are the oldest, and go first.
fn full_name(path: &Path) -> Result<String, Problem> {
    Ok(format!("{}-v{FULL_VERSION}-full.jpg", cache_key(path)?))
}

/// A preview that can't be decoded is damaged, as a RAW file's cut-off embedded JPEG is.
// photo-library: public, as PhotoDev decodes previews with it (the library's plan step 1.2).
pub fn decode(preview: &Preview) -> Result<DynamicImage, Problem> {
    image::load_from_memory_with_format(&preview.bytes, preview.format).map_err(|e| Problem::Damaged { cause: format!("can't decode the preview: {e}") })
}

// photo-library: public, as PhotoDev turns its own images with it (the library's plan step 1.2).
pub fn upright(image: DynamicImage, orientation: Orientation) -> DynamicImage {
    match orientation {
        Orientation::Normal => image,
        Orientation::Rotate90 => image.rotate90(),
        Orientation::Rotate180 => image.rotate180(),
        Orientation::Rotate270 => image.rotate270(),
    }
}

// photo-library: public, as PhotoDev encodes its JPEGs with it (the library's plan step 1.2).
pub fn encode(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&image.to_rgb8())
        .map_err(|e| format!("can't encode JPEG: {e}"))?;
    Ok(out.into_inner())
}

/// What the full-size photos kept in the cache may take, the newest made kept (record 0034): 2 GB,
/// about 100 photos of 42 MP, which take about 20 MB each.
const FULL_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Deletes the oldest `-full.jpg` files of `dir` until the rest take at most `limit` bytes. A file
/// that can't be deleted stays, and the others are still tried.
fn trim_full(dir: &Path, limit: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut kept: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with("-full.jpg"))
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), entry.path()))
        })
        .collect();
    let mut total: u64 = kept.iter().map(|file| file.1).sum();
    if total <= limit {
        return;
    }
    kept.sort_by_key(|file| file.0);
    for (_, len, path) in kept {
        if total <= limit {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// Reads any file as a photo developed from its RAW data, counting the reads.
    struct Developed(AtomicUsize);

    impl PhotoReader for Developed {
        fn read(&self, _: &Path, _: Want) -> Result<Photo, Problem> {
            self.0.fetch_add(1, Ordering::SeqCst);
            let jpeg = encode(&DynamicImage::new_rgb8(32, 24), 80)?;
            let preview = Preview { width: 32, height: 24, format: ImageFormat::Jpeg, bytes: jpeg, tagged_orientation: false, made: true };
            Ok(Photo { previews: vec![preview], no_preview: None, orientation: Orientation::Normal, info: PhotoInfo::default() })
        }
    }

    #[test]
    fn an_image_file_s_clipping_marks_are_kept_and_not_made_again() {
        let dir = std::env::temp_dir().join(format!("photomatch-thumbs-clipping-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let photo = dir.join("a.jpg");
        std::fs::write(&photo, b"a photo").unwrap();
        let cache = ThumbCache::new(dir.join("thumbnails")).unwrap();
        let reader = Developed(AtomicUsize::new(0));
        assert!(cache.cached_clipping(&photo, (32, 24)).is_none());

        let png = cache.clipping(&reader, &photo, (32, 24)).unwrap();
        let marks = image::load_from_memory_with_format(&png, ImageFormat::Png).unwrap();
        assert_eq!((marks.width(), marks.height()), (32, 24));
        // The made-up photo is black all over: a shadow everywhere, in blue.
        assert_eq!(marks.to_rgba8().get_pixel(5, 5).0, [0, 96, 255, 255]);
        assert_eq!(cache.clipping(&reader, &photo, (32, 24)).unwrap(), png);
        assert_eq!(cache.cached_clipping(&photo, (32, 24)).unwrap(), png);
        assert_eq!(reader.0.load(Ordering::SeqCst), 1, "read once");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_full_size_kept_by_an_older_development_is_made_again() {
        let dir = std::env::temp_dir().join(format!("photomatch-thumbs-full-version-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let photo = dir.join("a.ARW");
        std::fs::write(&photo, b"a photo").unwrap();
        let cache = ThumbCache::new(dir.join("thumbnails")).unwrap();
        // As the version before kept it: under the name without a version, its pink still in it.
        let older = dir.join("thumbnails").join(format!("{}-full.jpg", cache_key(&photo).unwrap()));
        std::fs::write(&older, b"made with LibRaw's brightening").unwrap();
        let reader = Developed(AtomicUsize::new(0));

        let (bytes, _) = cache.full(&reader, &photo).unwrap();
        assert_eq!(reader.0.load(Ordering::SeqCst), 1, "developed again");
        assert_ne!(bytes, b"made with LibRaw's brightening");
        assert!(full_name(&photo).unwrap().ends_with("-full.jpg"), "trimmed with the rest");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_zoom_finds_the_photo_made_ahead_and_reads_nothing() {
        let dir = std::env::temp_dir().join(format!("photomatch-thumbs-ahead-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let photo = dir.join("a.ARW");
        std::fs::write(&photo, b"a photo").unwrap();
        let cache = ThumbCache::new(dir.join("thumbnails")).unwrap();
        let reader = Developed(AtomicUsize::new(0));

        cache.prepare_full(&reader, &photo).unwrap();
        assert_eq!(reader.0.load(Ordering::SeqCst), 1, "made once, ahead");
        assert_eq!((cache.stats().prepared, cache.stats().fulls_made), (1, 1));
        cache.prepare_full(&reader, &photo).unwrap();
        assert_eq!(reader.0.load(Ordering::SeqCst), 1, "not made again");
        assert_eq!(cache.stats().prepared, 1, "and not counted again");
        assert!(cache.full(&reader, &photo).is_ok());
        assert_eq!(reader.0.load(Ordering::SeqCst), 1, "the zoom reads it from the cache");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_full_size_photos_kept_stay_within_their_limit_oldest_first() {
        let dir = std::env::temp_dir().join(format!("photomatch-thumbs-trim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let start = std::time::SystemTime::now() - std::time::Duration::from_secs(1000);
        for i in 0..5u64 {
            let file = std::fs::File::create(dir.join(format!("k{i}-full.jpg"))).unwrap();
            file.set_len(100).unwrap();
            file.set_modified(start + std::time::Duration::from_secs(i * 10)).unwrap();
        }
        // What isn't a full-size photo isn't counted or deleted.
        std::fs::write(dir.join("k0.jpg"), vec![0u8; 5000]).unwrap();
        std::fs::write(dir.join("k0.json"), b"{}").unwrap();

        trim_full(&dir, 250);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["k0.jpg", "k0.json", "k3-full.jpg", "k4-full.jpg"], "the three oldest went");
        trim_full(&dir, 1000);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 4, "within the limit, nothing goes");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Reads any file as a photo whose only preview is a small grey JPEG.
    struct GreyPhoto;

    impl PhotoReader for GreyPhoto {
        fn read(&self, _: &Path, _: Want) -> Result<Photo, Problem> {
            let jpeg = encode(&DynamicImage::new_rgb8(32, 24), 80)?;
            let preview = Preview { width: 32, height: 24, format: ImageFormat::Jpeg, bytes: jpeg, tagged_orientation: false, made: false };
            Ok(Photo { previews: vec![preview], no_preview: None, orientation: Orientation::Normal, info: PhotoInfo::default() })
        }
    }

    #[test]
    fn a_thumbnail_shows_when_its_cache_folder_is_gone() {
        let dir = std::env::temp_dir().join(format!("photomatch-thumbs-gone-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let photo = dir.join("a.ARW");
        std::fs::write(&photo, b"a photo").unwrap();
        let cache = ThumbCache::new(dir.join("thumbnails")).unwrap();

        // Emptied while the app runs, as a cleaner might.
        std::fs::remove_dir_all(dir.join("thumbnails")).unwrap();
        assert!(cache.thumbnail(&GreyPhoto, &photo).is_ok(), "the photo still shows");
        assert!(cache.info(&GreyPhoto, &photo).is_ok());
        assert!(dir.join("thumbnails").is_dir(), "and the folder is made again");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A photo moved keeps its thumbnail and details, under its new key, and isn't read again; a
    /// copy gets them too, the original keeping its own (record 0016).
    #[test]
    fn a_photo_moved_or_copied_keeps_its_thumbnail() {
        let dir = std::env::temp_dir().join(format!("photomatch-thumbs-moved-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Day 2")).unwrap();
        let photo = dir.join("a.ARW");
        std::fs::write(&photo, b"a photo").unwrap();
        let cache = ThumbCache::new(dir.join("thumbnails")).unwrap();
        let thumbnail = cache.thumbnail(&GreyPhoto, &photo).unwrap();
        cache.info(&GreyPhoto, &photo).unwrap();

        let key = ThumbCache::key_of(&photo).unwrap();
        let moved = dir.join("Day 2/a.ARW");
        std::fs::rename(&photo, &moved).unwrap();
        cache.moved(&[(key, moved.clone())], false);
        assert_eq!(cache.cached_thumbnail(&moved), Some(thumbnail.clone()), "kept under its new key");
        let made = cache.stats().made;
        cache.thumbnail(&GreyPhoto, &moved).unwrap();
        assert_eq!(cache.stats().made, made, "and not made again");

        let copy = dir.join("b.ARW");
        std::fs::copy(&moved, &copy).unwrap();
        let file = std::fs::File::options().write(true).open(&copy).unwrap();
        file.set_modified(std::fs::metadata(&moved).unwrap().modified().unwrap()).unwrap();
        drop(file);
        cache.moved(&[(ThumbCache::key_of(&moved).unwrap(), copy.clone())], true);
        assert_eq!(cache.cached_thumbnail(&copy), Some(thumbnail.clone()), "the copy has it");
        assert_eq!(cache.cached_thumbnail(&moved), Some(thumbnail), "and the photo still");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Two photos of one size and time that swap names keep their own thumbnails, though each
    /// one's new key is the other's old one (record 0017).
    #[test]
    fn photos_that_swap_names_keep_their_own_thumbnails() {
        let dir = std::env::temp_dir().join(format!("photomatch-thumbs-swapped-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.ARW"), dir.join("b.ARW"));
        std::fs::write(&a, b"one size").unwrap();
        std::fs::write(&b, b"one size").unwrap();
        let time = std::fs::metadata(&a).unwrap().modified().unwrap();
        std::fs::File::options().write(true).open(&b).unwrap().set_modified(time).unwrap();
        let cache = ThumbCache::new(dir.join("thumbnails")).unwrap();
        cache.thumbnail(&GreyPhoto, &a).unwrap();
        cache.thumbnail(&GreyPhoto, &b).unwrap();
        let (key_a, key_b) = (ThumbCache::key_of(&a).unwrap(), ThumbCache::key_of(&b).unwrap());
        let file = |key: &str| dir.join("thumbnails").join(format!("{key}.jpg"));
        // Told apart by their bytes in the cache.
        std::fs::write(file(&key_a), b"a's").unwrap();
        std::fs::write(file(&key_b), b"b's").unwrap();
        let between = dir.join("between");
        std::fs::rename(&a, &between).unwrap();
        std::fs::rename(&b, &a).unwrap();
        std::fs::rename(&between, &b).unwrap();
        cache.moved(&[(key_a, b.clone()), (key_b, a.clone())], false);
        assert_eq!(std::fs::read(file(&ThumbCache::key_of(&b).unwrap())).unwrap(), b"a's");
        assert_eq!(std::fs::read(file(&ThumbCache::key_of(&a).unwrap())).unwrap(), b"b's");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A photo with a position, which only its details are read for.
    struct PlacedPhoto;

    impl PhotoReader for PlacedPhoto {
        fn read(&self, _: &Path, want: Want) -> Result<Photo, Problem> {
            assert_eq!(want, Want::Details, "only the details are read again");
            let info = PhotoInfo { position: crate::photo::Position::new(37.8, -122.5), details_version: DETAILS_VERSION, width: 9, ..PhotoInfo::default() };
            Ok(Photo { previews: Vec::new(), no_preview: None, orientation: Orientation::Normal, info })
        }
    }

    /// Details cached before the position are read again once, without a preview; the thumbnail
    /// and the sizes stay (record 0010).
    #[test]
    fn details_cached_before_the_position_are_read_again_the_thumbnail_kept() {
        let dir = std::env::temp_dir().join(format!("photomatch-thumbs-version-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let photo = dir.join("a.ARW");
        std::fs::write(&photo, b"a photo").unwrap();
        let cache = ThumbCache::new(dir.join("thumbnails")).unwrap();
        // As an older version cached them: version 0, no position.
        let old = cache.info(&GreyPhoto, &photo).unwrap();
        assert_eq!((old.details_version, old.position), (0, None));
        let thumbnail = cache.thumbnail(&GreyPhoto, &photo).unwrap();

        let info = cache.info(&PlacedPhoto, &photo).unwrap();
        assert_eq!(info.position, crate::photo::Position::new(37.8, -122.5));
        assert_eq!(info.details_version, DETAILS_VERSION);
        assert_eq!((info.thumb_width, info.thumb_height, info.width), (old.thumb_width, old.thumb_height, old.width), "the sizes kept");
        assert_eq!(cache.thumbnail(&PlacedPhoto, &photo).unwrap(), thumbnail, "the thumbnail kept, not made again");
        // Read again once: now as cached, GreyPhoto not asked.
        assert_eq!(cache.info(&GreyPhoto, &photo).unwrap().position, info.position);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
