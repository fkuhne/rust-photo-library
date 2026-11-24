//! Photos moved or copied to another folder, with the JPEG shot beside them and their XMP sidecar,
//! never over a file there, and moved back (record 0016; PhotoMatch's plan step 2.71 and its
//! record 0056). On the same drive a move renames each file. To another drive, each file is copied
//! beside its new name under a hidden one, checked byte for byte, then given its name; a move then
//! sends the originals to the Trash, never deletes them outright, and leaves them where there's no
//! Trash. Nothing here touches the ratings or the thumbnails, which an app moves with the photo.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::problem::Problem;
use crate::trashing::{self, Trashed};

/// Whether a photo's files are moved, or copied and left where they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    Move,
    Copy,
}

/// What became of a photo's files, to say so and to put them back.
#[derive(Debug, Clone)]
pub struct Placed {
    /// Where the photo was, and where it is now, or its copy.
    pub photo: PathBuf,
    pub to: PathBuf,
    /// The JPEG shot beside it, at its new place.
    pub companion: Option<PathBuf>,
    /// Each file, from where to where.
    files: Vec<(PathBuf, PathBuf)>,
    pub done: Done,
}

#[derive(Debug, Clone)]
pub enum Done {
    /// Moved on the same drive, by renaming.
    Moved,
    /// Copied; the originals stay.
    Copied,
    /// Moved to another drive: copied, checked, and the originals sent to the Trash.
    MovedAcross { trashed: Trashed },
    /// Moved to another drive, copied and checked, but the originals stay where they were, as they
    /// couldn't go to the Trash: a Windows card has no Recycle Bin, say.
    OriginalsKept { cause: String },
}

impl Placed {
    /// The files placed, at their new places.
    pub fn files(&self) -> Vec<PathBuf> {
        self.files.iter().map(|(_, to)| to.clone()).collect()
    }

    /// Each file, from where to where: for an app to move its records of them.
    pub fn moves(&self) -> &[(PathBuf, PathBuf)] {
        &self.files
    }
}

/// Each of a photo's files, as `trashing::files_of` lists them, and its name in `into`: its own,
/// or with `-number` after its stem when `number` is given, the same for all, so that the JPEG and
/// the sidecar still go with the photo.
pub fn names_in(photo: &Path, companion: Option<&Path>, into: &Path, number: Option<u32>) -> Vec<(PathBuf, PathBuf)> {
    trashing::files_of(photo, companion)
        .into_iter()
        .map(|file| {
            let name = match number {
                None => file.file_name().unwrap_or_default().to_os_string(),
                Some(n) => {
                    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
                    match file.extension() {
                        Some(ext) => format!("{stem}-{n}.{}", ext.to_string_lossy()).into(),
                        None => format!("{stem}-{n}").into(),
                    }
                }
            };
            let to = into.join(name);
            (file, to)
        })
        .collect()
}

/// Whether a file of `into` already has the name one of the photo's files would take.
pub fn clashes(photo: &Path, companion: Option<&Path>, into: &Path) -> bool {
    names_in(photo, companion, into, None).iter().any(|(_, to)| to.exists())
}

/// The smallest number from 2 that gives every one of the photo's files a name free in `into`:
/// "Keep Both", as `DSC0001-2.ARW`.
pub fn free_number(photo: &Path, companion: Option<&Path>, into: &Path) -> u32 {
    (2..).find(|&n| names_in(photo, companion, into, Some(n)).iter().all(|(_, to)| !to.exists())).unwrap_or(2)
}

/// Whether two folders are on the same drive, where a move is a rename: by the device on a Mac, as
/// the system numbers it; by the drive letter or the network share on Windows (unchecked for a
/// drive mounted in a folder).
pub fn same_drive(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        matches!((std::fs::metadata(a), std::fs::metadata(b)), (Ok(a), Ok(b)) if a.dev() == b.dev())
    }
    #[cfg(not(unix))]
    {
        let root = |p: &Path| match p.components().next() {
            Some(std::path::Component::Prefix(prefix)) => Some(prefix.as_os_str().to_string_lossy().to_lowercase()),
            _ => None,
        };
        root(a).is_some() && root(a) == root(b)
    }
}

/// Moves or copies a photo's files into `into`, together: if one can't go, those gone come back,
/// or the copies made go, and the photo stays whole. `number` names them as `free_number` says,
/// for "Keep Both". `copied` is told the bytes of each file copied, for an app's progress. A file
/// already there with one of their names is never written over: the photo stays, and says so.
pub fn place(photo: &Path, companion: Option<&Path>, into: &Path, how: How, number: Option<u32>, copied: &dyn Fn(u64)) -> Result<Placed, Problem> {
    let files = names_in(photo, companion, into, number);
    let failed = |cause: String| match how {
        How::Move => Problem::NotMoved { photo: name_of(photo), cause },
        How::Copy => Problem::NotCopied { photo: name_of(photo), cause },
    };
    if let Some((_, to)) = files.iter().find(|(from, to)| from == to) {
        return Err(failed(format!("{} is already in {}", name_of(to), into.display())));
    }
    if let Some((_, to)) = files.iter().find(|(_, to)| to.exists()) {
        return Err(failed(format!("a file named {} is already in {}", name_of(to), into.display())));
    }
    let placed = |done| Placed {
        photo: photo.to_path_buf(),
        to: files[0].1.clone(),
        companion: companion.and_then(|c| files.iter().find(|(from, _)| from == c)).map(|(_, to)| to.clone()),
        files: files.clone(),
        done,
    };
    if how == How::Move && same_drive(photo.parent().unwrap_or(photo), into) {
        match rename_all(&files) {
            Ok(()) => return Ok(placed(Done::Moved)),
            // A drive mounted in a folder, say: copied instead, as to another drive.
            Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {}
            Err(error) => return Err(failed(error.to_string())),
        }
    }
    copy_all(&files, copied).map_err(failed)?;
    match how {
        How::Copy => Ok(placed(Done::Copied)),
        How::Move => Ok(placed(match trashing::to_trash(photo, companion) {
            Ok(trashed) => Done::MovedAcross { trashed },
            Err(problem) => Done::OriginalsKept { cause: problem.cause() },
        })),
    }
}

/// Undo: a photo moved back where it was, its files renamed back, or brought back from the Trash
/// with its copies sent there; a photo copied has its copies sent to the Trash. Nothing deleted
/// outright.
pub fn put_back(placed: &Placed) -> Result<(), Problem> {
    let failed = |cause: String| Problem::NotMovedBack { photo: name_of(&placed.photo), cause };
    match &placed.done {
        Done::Moved => {
            let back: Vec<(PathBuf, PathBuf)> = placed.files.iter().map(|(from, to)| (to.clone(), from.clone())).collect();
            rename_all(&back).map_err(|e| failed(e.to_string()))
        }
        Done::Copied | Done::OriginalsKept { .. } => trashing::to_trash(&placed.to, placed.companion.as_deref()).map(|_| ()).map_err(|p| failed(p.cause())),
        Done::MovedAcross { trashed } => {
            trashing::put_back(trashed).map_err(|p| failed(p.cause()))?;
            trashing::to_trash(&placed.to, placed.companion.as_deref()).map(|_| ()).map_err(|p| failed(p.cause()))
        }
    }
}

/// Renames each file, never over one; if one can't be, those renamed go back.
fn rename_all(files: &[(PathBuf, PathBuf)]) -> std::io::Result<()> {
    for (done, (from, to)) in files.iter().enumerate() {
        if let Err(error) = os::rename_no_replace(from, to) {
            for (from, to) in files[..done].iter().rev() {
                let _ = os::rename_no_replace(to, from);
            }
            let error = std::io::Error::new(error.kind(), format!("can't move {} to {}: {error}", from.display(), to.display()));
            return Err(error);
        }
    }
    Ok(())
}

/// Copies each file, checked; if one can't be, the copies made go.
fn copy_all(files: &[(PathBuf, PathBuf)], copied: &dyn Fn(u64)) -> Result<(), String> {
    for (done, (from, to)) in files.iter().enumerate() {
        match copy_checked(from, to) {
            Ok(bytes) => copied(bytes),
            Err(cause) => {
                for (_, made) in &files[..done] {
                    let _ = std::fs::remove_file(made);
                }
                return Err(cause);
            }
        }
    }
    Ok(())
}

/// Copies `from` to `to` under a hidden name beside it, keeps its time of change, checks the copy
/// byte for byte by its hash, then gives it its name, never over a file. Says the bytes copied. A
/// copy that fails is removed: it's PhotoMatch's own, never a photographer's file.
fn copy_checked(from: &Path, to: &Path) -> Result<u64, String> {
    static PARTS: AtomicU64 = AtomicU64::new(0);
    let part = to.with_file_name(format!(".photomatch-{}-{}.part", std::process::id(), PARTS.fetch_add(1, Ordering::Relaxed)));
    let result = (|| {
        let bytes = std::fs::copy(from, &part).map_err(|e| format!("can't copy {} to {}: {e}", from.display(), to.display()))?;
        let modified = std::fs::metadata(from).and_then(|m| m.modified()).map_err(|e| format!("can't read {}: {e}", from.display()))?;
        let copy = std::fs::File::options().write(true).open(&part).map_err(|e| format!("can't open the copy of {}: {e}", from.display()))?;
        copy.set_modified(modified).map_err(|e| format!("can't keep the time of {} on its copy: {e}", from.display()))?;
        copy.sync_all().map_err(|e| format!("can't write the copy of {} through: {e}", from.display()))?;
        drop(copy);
        if hash_of(from)? != hash_of(&part)? {
            return Err(format!("the copy of {} differs from it", from.display()));
        }
        os::rename_no_replace(&part, to).map_err(|e| format!("can't name the copy {}: {e}", to.display()))?;
        Ok(bytes)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

fn hash_of(path: &Path) -> Result<blake3::Hash, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0; 1 << 20];
    loop {
        let read = file.read(&mut buffer).map_err(|e| format!("can't read {}: {e}", path.display()))?;
        if read == 0 {
            return Ok(hasher.finalize());
        }
        hasher.update(&buffer[..read]);
    }
}

fn name_of(path: &Path) -> String {
    path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned()
}

/// Renaming that never writes over a file: `AlreadyExists` if `to` is there, `CrossesDevices`
/// if it's on another drive.
#[cfg(target_os = "macos")]
mod os {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    /// `renamex_np` with `RENAME_EXCL`, which returns `EEXIST` rather than replace (its manual);
    /// where the file system doesn't support it, `ENOTSUP`, a look first, then a plain rename.
    pub fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
        let c = |p: &Path| CString::new(p.as_os_str().as_bytes()).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e));
        let (c_from, c_to) = (c(from)?, c(to)?);
        // SAFETY: both are valid, NUL-terminated paths that live through the call.
        if unsafe { libc::renamex_np(c_from.as_ptr(), c_to.as_ptr(), libc::RENAME_EXCL) } == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ENOTSUP) {
            return Err(error);
        }
        if to.exists() {
            return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("{} is there", to.display())));
        }
        std::fs::rename(from, to)
    }
}

#[cfg(windows)]
mod os {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows::Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW};
    use windows::core::PCWSTR;

    /// `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`, which never replaces, and without
    /// `MOVEFILE_COPY_ALLOWED`, which another drive needs (Microsoft's MoveFileExW).
    pub fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
        let wide = |p: &Path| p.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<u16>>();
        let (w_from, w_to) = (wide(from), wide(to));
        // SAFETY: both are valid, NUL-terminated wide paths that live through the call.
        unsafe { MoveFileExW(PCWSTR(w_from.as_ptr()), PCWSTR(w_to.as_ptr()), MOVE_FILE_FLAGS(0)) }.map_err(|e| std::io::Error::from_raw_os_error((e.code().0 & 0xFFFF) as i32))
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod os {
    use std::path::Path;

    /// A hard link, which fails when `to` is there, then the old name removed; a look first, then
    /// a plain rename, where links aren't supported.
    pub fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
        match std::fs::hard_link(from, to) {
            Ok(()) => std::fs::remove_file(from),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists || error.kind() == std::io::ErrorKind::CrossesDevices => Err(error),
            Err(_) if to.exists() => Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("{} is there", to.display()))),
            Err(_) => std::fs::rename(from, to),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("photo-library-moving-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A RAW file with its JPEG and sidecar in `dir`, each its own bytes.
    fn shoot(dir: &Path, stem: &str) -> (PathBuf, PathBuf) {
        let (raw, jpeg) = (dir.join(format!("{stem}.ARW")), dir.join(format!("{stem}.JPG")));
        std::fs::write(&raw, format!("{stem} raw")).unwrap();
        std::fs::write(&jpeg, format!("{stem} jpeg")).unwrap();
        std::fs::write(dir.join(format!("{stem}.xmp")), format!("{stem} sidecar")).unwrap();
        (raw, jpeg)
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn a_photo_moves_with_its_jpeg_and_sidecar_and_moves_back() {
        let dir = folder("move");
        let into = dir.join("Day 2");
        std::fs::create_dir_all(&into).unwrap();
        let (raw, jpeg) = shoot(&dir, "DSC1");
        let modified = std::fs::metadata(&raw).unwrap().modified().unwrap();

        let placed = place(&raw, Some(&jpeg), &into, How::Move, None, &|_| {}).unwrap();
        assert!(matches!(placed.done, Done::Moved), "a rename on the same drive: {placed:?}");
        assert_eq!(names(&into), ["DSC1.ARW", "DSC1.JPG", "DSC1.xmp"]);
        assert_eq!(names(&dir), ["Day 2"], "nothing left behind");
        assert_eq!((placed.to.clone(), placed.companion.clone()), (into.join("DSC1.ARW"), Some(into.join("DSC1.JPG"))));
        assert_eq!(std::fs::read_to_string(into.join("DSC1.xmp")).unwrap(), "DSC1 sidecar");
        assert_eq!(std::fs::metadata(into.join("DSC1.ARW")).unwrap().modified().unwrap(), modified, "its time kept");

        put_back(&placed).unwrap();
        assert_eq!(names(&dir), ["DSC1.ARW", "DSC1.JPG", "DSC1.xmp", "Day 2"]);
        assert!(names(&into).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_there_is_never_written_over_and_keep_both_numbers_them() {
        let dir = folder("clash");
        let into = dir.join("Day 2");
        std::fs::create_dir_all(&into).unwrap();
        let (raw, jpeg) = shoot(&dir, "DSC1");
        std::fs::write(into.join("DSC1.JPG"), "another camera's").unwrap();

        assert!(clashes(&raw, Some(&jpeg), &into));
        let refused = place(&raw, Some(&jpeg), &into, How::Move, None, &|_| {}).unwrap_err();
        assert!(matches!(&refused, Problem::NotMoved { photo, .. } if photo == "DSC1.ARW"), "{refused:?}");
        assert_eq!(std::fs::read_to_string(into.join("DSC1.JPG")).unwrap(), "another camera's", "untouched");
        assert_eq!(names(&dir), ["DSC1.ARW", "DSC1.JPG", "DSC1.xmp", "Day 2"], "and the photo whole where it was");

        std::fs::write(into.join("DSC1-2.xmp"), "taken too").unwrap();
        let number = free_number(&raw, Some(&jpeg), &into);
        assert_eq!(number, 3, "the first number free for all three files");
        let placed = place(&raw, Some(&jpeg), &into, How::Copy, Some(number), &|_| {}).unwrap();
        assert_eq!(names(&into), ["DSC1-2.xmp", "DSC1-3.ARW", "DSC1-3.JPG", "DSC1-3.xmp", "DSC1.JPG"]);
        assert_eq!(std::fs::read_to_string(&placed.to).unwrap(), "DSC1 raw");
        assert_eq!(names(&dir).len(), 4, "a copy leaves the originals");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copy_is_checked_keeps_its_time_and_says_its_bytes() {
        let dir = folder("copy");
        let into = dir.join("Copies");
        std::fs::create_dir_all(&into).unwrap();
        let (raw, _) = shoot(&dir, "DSC2");
        let modified = std::fs::metadata(&raw).unwrap().modified().unwrap();
        let bytes = Cell::new(0);
        let placed = place(&raw, None, &into, How::Copy, None, &|n| bytes.set(bytes.get() + n)).unwrap();
        assert!(matches!(placed.done, Done::Copied));
        assert_eq!(names(&into), ["DSC2.ARW", "DSC2.xmp"], "no hidden part left");
        assert_eq!(bytes.get(), "DSC2 raw".len() as u64 + "DSC2 sidecar".len() as u64);
        assert_eq!(std::fs::metadata(&placed.to).unwrap().modified().unwrap(), modified);
        assert_eq!(placed.moves().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_photo_already_in_the_folder_stays() {
        let dir = folder("same");
        let (raw, _) = shoot(&dir, "DSC3");
        assert!(place(&raw, None, &dir, How::Move, None, &|_| {}).is_err());
        assert_eq!(names(&dir), ["DSC3.ARW", "DSC3.JPG", "DSC3.xmp"]);
        assert!(same_drive(&dir, &std::env::temp_dir()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// To another drive, the copies checked and the originals to the Trash, then all back: only
    /// with `PHOTO_LIBRARY_OTHER_DRIVE` naming a folder on another drive, a disk image say.
    #[test]
    fn a_photo_moved_to_another_drive_is_copied_and_its_originals_go_to_the_trash() {
        let Some(other) = std::env::var_os("PHOTO_LIBRARY_OTHER_DRIVE").map(PathBuf::from) else { return };
        let dir = folder("across");
        let into = other.join(format!("photo-library-moving-{}", std::process::id()));
        std::fs::create_dir_all(&into).unwrap();
        assert!(!same_drive(&dir, &into), "{} is on the same drive", other.display());
        let (raw, jpeg) = shoot(&dir, "DSC4");
        let placed = place(&raw, Some(&jpeg), &into, How::Move, None, &|_| {}).unwrap();
        assert!(matches!(placed.done, Done::MovedAcross { .. }), "{placed:?}");
        assert_eq!(names(&into), ["DSC4.ARW", "DSC4.JPG", "DSC4.xmp"]);
        assert!(names(&dir).is_empty(), "the originals in the Trash");
        put_back(&placed).unwrap();
        assert_eq!(names(&dir), ["DSC4.ARW", "DSC4.JPG", "DSC4.xmp"], "back from the Trash");
        assert!(names(&into).is_empty(), "the copies sent to the Trash");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&into);
    }
}
