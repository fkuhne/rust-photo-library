//! A folder's photos, with its subfolders: RAW, DNG and image files, as the readers take them, a
//! JPEG beside a RAW file of the same name shown once, as the RAW file (record 0007). PhotoMatch's
//! listing, from its `lib.rs` at commit 391fd74, with PhotoDev's `raw` flag and its leaving out of
//! hidden files and folders, from its `folder.rs` at commit d95ff89.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::photo;
use crate::problem::Problem;
use crate::raw;

/// One photo in a folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: PathBuf,
    /// The JPEG shot beside a RAW file, shown as one photo with it (PhotoMatch's record 0011).
    pub companion: Option<PathBuf>,
    /// Whether it's a RAW file, read through LibRaw, rather than an image file (PhotoDev's).
    pub raw: bool,
}

/// A folder's photos, and what in it couldn't be opened.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// The photos, in the order of their paths.
    pub photos: Vec<Entry>,
    /// Subfolders and files that couldn't be opened, so that their photos are missing: each path,
    /// with why, for an app to say (PhotoMatch's plan step 2.35).
    pub unlisted: Vec<String>,
}

/// The photos in `folder` and its subfolders, sorted by path; a RAW+JPEG pair is one photo.
/// Hidden files and folders, whose names start with a dot, are left out, and all that's inside a
/// hidden folder: the `._` files macOS writes beside photos on a memory card, for one (unchecked).
/// The folder opened is listed even if its own name starts with a dot. A folder that isn't there,
/// or can't be listed, is a problem of its own kind (PhotoMatch's plan step 2.35).
pub fn list(folder: &Path) -> Result<Listing, Problem> {
    if let Err(error) = std::fs::read_dir(folder) {
        let name = folder.file_name().unwrap_or(folder.as_os_str()).to_string_lossy().into_owned();
        return Err(match error.kind() {
            std::io::ErrorKind::NotFound => Problem::FolderMissing { folder: name },
            std::io::ErrorKind::PermissionDenied => Problem::FolderNotAllowed { folder: name },
            _ => Problem::FolderUnreadable { folder: name, cause: format!("can't list {}: {error}", folder.display()) },
        });
    }
    let mut unlisted = Vec::new();
    let mut files = Vec::new();
    // `walkdir`'s own way of leaving hidden entries out, without going into hidden folders; each
    // entry comes as its own result, so a folder that can't be read doesn't end the walk.
    for entry in walkdir::WalkDir::new(folder).into_iter().filter_entry(|entry| entry.depth() == 0 || !is_hidden(entry)) {
        match entry {
            Ok(entry) if entry.file_type().is_file() && photo::is_supported(entry.path()) => files.push(entry.into_path()),
            Ok(_) => {}
            Err(error) => unlisted.push(match error.path() {
                Some(path) => format!("{}: {error}", path.display()),
                None => error.to_string(),
            }),
        }
    }
    files.sort();
    let photos = photo::pair(files).into_iter().map(|(path, companion)| Entry { raw: raw::is_raw(&path), path, companion }).collect();
    Ok(Listing { photos, unlisted })
}

/// A file or folder whose name starts with a dot.
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name().to_str().is_some_and(|name| name.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder in the system's temporary folder, named for the test.
    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("photo-library-folder-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The photos listed, as their paths within `dir`, whether each has a companion, and whether
    /// it's a RAW file.
    fn shown(listing: &Listing, dir: &Path) -> Vec<(String, bool, bool)> {
        listing
            .photos
            .iter()
            .map(|e| (e.path.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"), e.companion.is_some(), e.raw))
            .collect()
    }

    /// PhotoDev's test, without its edits files, and with a `._` file beside a photo.
    #[test]
    fn lists_photos_with_subfolders_and_pairs_and_leaves_hidden_ones_out() {
        let dir = folder("photos");
        std::fs::create_dir_all(dir.join("day 2")).unwrap();
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        for name in ["DSC1.ARW", "DSC1.JPG", "._DSC1.ARW", "IMG2.jpg", "notes.txt", "day 2/DSC3.NEF", ".hidden/DSC4.ARW"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let listing = list(&dir).unwrap();
        assert_eq!(
            shown(&listing, &dir),
            [("DSC1.ARW".to_string(), true, true), ("IMG2.jpg".to_string(), false, false), ("day 2/DSC3.NEF".to_string(), false, true)]
        );
        assert!(listing.unlisted.is_empty(), "{:?}", listing.unlisted);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_folder_opened_is_listed_even_if_its_name_starts_with_a_dot() {
        let dir = folder("dot");
        let inside = dir.join(".photos");
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::write(inside.join("DSC1.ARW"), b"x").unwrap();
        assert_eq!(shown(&list(&inside).unwrap(), &inside), [("DSC1.ARW".to_string(), false, true)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_folder_is_missing() {
        assert!(matches!(list(Path::new("/no/such/folder")), Err(Problem::FolderMissing { .. })));
    }

    /// A subfolder that can't be read is reported, and the others are still listed. Not where the
    /// tests run with the rights to read it anyway, as root does.
    #[cfg(unix)]
    #[test]
    fn a_subfolder_that_cant_be_read_is_reported_and_the_rest_listed() {
        use std::os::unix::fs::PermissionsExt;
        let dir = folder("locked");
        std::fs::create_dir_all(dir.join("locked")).unwrap();
        std::fs::write(dir.join("locked/DSC2.ARW"), b"x").unwrap();
        std::fs::write(dir.join("DSC1.ARW"), b"x").unwrap();
        std::fs::set_permissions(dir.join("locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
        let readable_anyway = std::fs::read_dir(dir.join("locked")).is_ok();
        let listing = list(&dir).unwrap();
        std::fs::set_permissions(dir.join("locked"), std::fs::Permissions::from_mode(0o755)).unwrap();
        if !readable_anyway {
            assert_eq!(shown(&listing, &dir), [("DSC1.ARW".to_string(), false, true)]);
            assert!(listing.unlisted.len() == 1 && listing.unlisted[0].contains("locked"), "{:?}", listing.unlisted);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
