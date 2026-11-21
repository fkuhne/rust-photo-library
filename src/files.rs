//! Writing files: the app's own thumbnails, details and ratings, and the XMP sidecars beside
//! RAW files (record 0014). Never the photos (record 0004).

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Writes through a temporary file, so a crash never leaves half a file, and an app reading it
/// never sees one. Each write gets its own temporary name, as two requests can write the same
/// file at once. The name is hidden, as a sidecar's sits in the photographer's folder.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_through_temporary(path, bytes, false)
}

/// The same, and on the disk before it takes the old file's place, so that a power cut leaves
/// the old file or the new one, whole: for what can't be made again, the ratings and the
/// sidecars. The thumbnails can, so they skip the wait.
pub fn write_durably(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_through_temporary(path, bytes, true)
}

fn write_through_temporary(path: &Path, bytes: &[u8], durably: bool) -> Result<(), String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", NEXT.fetch_add(1, Ordering::Relaxed)));
    let written = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        if durably {
            file.sync_all()?;
        }
        drop(file);
        std::fs::rename(&tmp, path)
    })();
    written.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("can't write {}: {e}", path.display())
    })
}
