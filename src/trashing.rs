//! Photos moved to the Trash, the Recycle Bin on Windows, with the JPEG shot beside them and their
//! XMP sidecar, and put back (record 0015; PhotoMatch's plan step 2.69 and its record 0053).
//! Nothing here deletes a file outright, nor touches the ratings, which an app keeps by the photo's
//! path for its return.

use std::path::{Path, PathBuf};

use crate::problem::Problem;
use crate::sidecar;

/// A photo's files gone to the Trash, and where each went, to put them back.
#[derive(Debug, Clone)]
pub struct Trashed {
    pub photo: PathBuf,
    files: Vec<Gone>,
}

impl Trashed {
    /// The files that went, where they were.
    pub fn files(&self) -> Vec<PathBuf> {
        self.files.iter().map(|gone| gone.original.clone()).collect()
    }
}

/// One file gone to the Trash.
#[derive(Debug, Clone)]
struct Gone {
    original: PathBuf,
    /// On a Mac, the place it took in the Trash, to move it back from.
    #[cfg(target_os = "macos")]
    in_trash: PathBuf,
    /// Elsewhere, about when it went, in seconds since 1970, to find it among the Trash's items.
    #[cfg(not(target_os = "macos"))]
    since: i64,
}

/// A photo's files: itself, the JPEG shot beside it, and its sidecar if it has one. A sidecar
/// beside a file that gets none, as a JPEG alone, isn't the photo's, and stays.
pub fn files_of(photo: &Path, companion: Option<&Path>) -> Vec<PathBuf> {
    let mut files = vec![photo.to_path_buf()];
    files.extend(companion.map(Path::to_path_buf));
    let sidecar = sidecar::path_for(photo);
    if sidecar::has_sidecar(photo) && sidecar.is_file() && !files.contains(&sidecar) {
        files.push(sidecar);
    }
    files
}

/// Moves a photo's files to the Trash, together: if one can't go, those gone come back, and the
/// photo stays whole.
pub fn to_trash(photo: &Path, companion: Option<&Path>) -> Result<Trashed, Problem> {
    let files = files_of(photo, companion);
    os::to_trash(&files)
        .map(|files| Trashed { photo: photo.to_path_buf(), files })
        .map_err(|cause| Problem::NotTrashed { photo: name_of(photo), cause })
}

/// Moves a photo's files back from the Trash to where they were. A file back at its place by
/// other means is left as it is.
pub fn put_back(trashed: &Trashed) -> Result<(), Problem> {
    let missing: Vec<Gone> = trashed.files.iter().filter(|gone| !gone.original.exists()).cloned().collect();
    if missing.is_empty() {
        return Ok(());
    }
    os::put_back(&missing).map_err(|cause| Problem::NotPutBack { photo: name_of(&trashed.photo), cause })
}

fn name_of(path: &Path) -> String {
    path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned()
}

// macOS: through NSFileManager, which asks no permission and gives each file's place in the Trash.
// The `trash` crate's Finder way asks to control Finder; its NSFileManager way keeps no place.
#[cfg(target_os = "macos")]
mod os {
    use std::path::{Path, PathBuf};

    use objc2::rc::Retained;
    use objc2_foundation::{NSFileManager, NSString, NSURL};

    use super::Gone;

    fn url_of(path: &Path) -> Retained<NSURL> {
        NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
    }

    pub fn to_trash(files: &[PathBuf]) -> Result<Vec<Gone>, String> {
        let manager = NSFileManager::defaultManager();
        let mut gone = Vec::new();
        for file in files {
            let mut resulting: Option<Retained<NSURL>> = None;
            let done = manager.trashItemAtURL_resultingItemURL_error(&url_of(file), Some(&mut resulting));
            let in_trash = resulting.and_then(|url| url.path()).map(|path| PathBuf::from(path.to_string()));
            match (done, in_trash) {
                (Ok(()), Some(in_trash)) => gone.push(Gone { original: file.clone(), in_trash }),
                (result, _) => {
                    let cause = match result {
                        Err(error) => error.localizedDescription().to_string(),
                        Ok(()) => "the Trash gave no place for it".into(),
                    };
                    // Those gone come back, so that the photo goes whole or not at all.
                    let _ = put_back(&gone);
                    return Err(format!("can't move {} to the Trash: {cause}", file.display()));
                }
            }
        }
        Ok(gone)
    }

    pub fn put_back(files: &[Gone]) -> Result<(), String> {
        let manager = NSFileManager::defaultManager();
        let mut failed = Vec::new();
        for gone in files {
            if let Err(error) = manager.moveItemAtURL_toURL_error(&url_of(&gone.in_trash), &url_of(&gone.original)) {
                failed.push(format!("{}: {}", gone.original.display(), error.localizedDescription()));
            }
        }
        if failed.is_empty() { Ok(()) } else { Err(format!("can't put back from the Trash {}", failed.join("; "))) }
    }
}

// Windows and Linux: through the `trash` crate. On Windows, each call runs on a thread of its own:
// IFileOperation needs a single-threaded apartment, and the crate panics on a thread set up
// otherwise, as `os_image` sets the app's threads it decodes on.
#[cfg(not(target_os = "macos"))]
mod os {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::Gone;

    /// The seconds since 1970, a little early, as the Trash may round its own.
    fn now() -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) - 2
    }

    fn on_own_thread<T: Send + 'static>(job: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
        std::thread::spawn(job).join().unwrap_or_else(|_| Err("the Trash's work stopped unexpectedly".into()))
    }

    pub fn to_trash(files: &[PathBuf]) -> Result<Vec<Gone>, String> {
        let files = files.to_vec();
        on_own_thread(move || {
            let since = now();
            match trash::delete_all(&files) {
                Ok(()) => Ok(files.into_iter().map(|original| Gone { original, since }).collect()),
                Err(error) => {
                    // Those gone come back, so that the photo goes whole or not at all.
                    let gone: Vec<Gone> = files.iter().filter(|f| !f.exists()).map(|original| Gone { original: original.clone(), since }).collect();
                    let _ = restore(&gone);
                    Err(format!("can't move {} to the Recycle Bin: {error}", files[0].display()))
                }
            }
        })
    }

    pub fn put_back(files: &[Gone]) -> Result<(), String> {
        let files = files.to_vec();
        on_own_thread(move || restore(&files))
    }

    /// Each file's newest item in the Trash since it went, restored.
    fn restore(files: &[Gone]) -> Result<(), String> {
        if files.is_empty() {
            return Ok(());
        }
        let items = trash::os_limited::list().map_err(|e| format!("can't read the Recycle Bin: {e}"))?;
        let mut found = Vec::new();
        for gone in files {
            let newest = items
                .iter()
                .filter(|item| item.original_parent.join(&item.name) == gone.original && item.time_deleted >= gone.since)
                .max_by_key(|item| item.time_deleted);
            match newest {
                Some(item) => found.push(item.clone()),
                None => return Err(format!("{} isn't in the Recycle Bin", gone.original.display())),
            }
        }
        trash::os_limited::restore_all(found).map_err(|e| format!("can't put back from the Recycle Bin: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("photo-library-trashing-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_photo_s_files_are_itself_its_jpeg_and_its_sidecar_and_no_other() {
        let dir = folder("files");
        for name in ["DSC1.ARW", "DSC1.JPG", "DSC1.xmp", "Lone.jpg", "Lone.xmp", "DSC2.ARW"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        assert_eq!(files_of(&dir.join("DSC1.ARW"), Some(&dir.join("DSC1.JPG"))), [dir.join("DSC1.ARW"), dir.join("DSC1.JPG"), dir.join("DSC1.xmp")]);
        assert_eq!(files_of(&dir.join("DSC2.ARW"), None), [dir.join("DSC2.ARW")], "no sidecar yet");
        assert_eq!(files_of(&dir.join("Lone.jpg"), None), [dir.join("Lone.jpg")], "a JPEG's sidecar isn't one PhotoMatch writes, and stays");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The system's own Trash: the photo's files go there, and come back byte for byte.
    #[test]
    fn a_photo_goes_to_the_trash_with_its_jpeg_and_sidecar_and_comes_back() {
        let dir = folder("trash");
        let (raw, jpeg, xmp) = (dir.join("PL trash check.ARW"), dir.join("PL trash check.JPG"), dir.join("PL trash check.xmp"));
        std::fs::write(&raw, b"raw bytes").unwrap();
        std::fs::write(&jpeg, b"jpeg bytes").unwrap();
        std::fs::write(&xmp, b"<x:xmpmeta/>").unwrap();
        let trashed = to_trash(&raw, Some(&jpeg)).unwrap();
        assert_eq!(trashed.files(), [raw.clone(), jpeg.clone(), xmp.clone()]);
        assert!(!raw.exists() && !jpeg.exists() && !xmp.exists(), "all three gone from the folder");
        put_back(&trashed).unwrap();
        assert_eq!((std::fs::read(&raw).unwrap(), std::fs::read(&jpeg).unwrap(), std::fs::read(&xmp).unwrap()), (b"raw bytes".to_vec(), b"jpeg bytes".to_vec(), b"<x:xmpmeta/>".to_vec()));
        // A file back by other means is left as it is; the copy in the Trash is the test's to clear.
        let again = to_trash(&raw, None).unwrap();
        std::fs::write(&raw, b"another").unwrap();
        put_back(&again).unwrap();
        assert_eq!(std::fs::read(&raw).unwrap(), b"another");
        #[cfg(target_os = "macos")]
        std::fs::remove_file(&again.files[0].in_trash).unwrap();
        #[cfg(not(target_os = "macos"))]
        {
            let items = trash::os_limited::list().unwrap().into_iter().filter(|i| i.original_parent.join(&i.name) == raw).collect::<Vec<_>>();
            trash::os_limited::purge_all(items).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_photo_that_isn_t_there_can_t_go_and_says_so() {
        let dir = folder("missing");
        let problem = to_trash(&dir.join("Gone.ARW"), None).unwrap_err();
        assert!(matches!(&problem, Problem::NotTrashed { photo, .. } if photo == "Gone.ARW"), "{problem:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
