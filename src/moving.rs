//! Photos moved or copied to another folder, with the JPEG shot beside them and their XMP sidecar,
//! never over a file there, and moved back (record 0016; PhotoMatch's plan step 2.71 and its
//! record 0056). On the same drive a move renames each file. To another drive, each file is copied
//! beside its new name under a hidden one, checked byte for byte, then given its name; a move then
//! sends the originals to the Trash, never deletes them outright, and leaves them where there's no
//! Trash. Photos renamed in their own folders, in an order that frees each name first (record 0017;
//! PhotoMatch's plan step 2.72 and its record 0057). Nothing here touches the ratings or the
//! thumbnails, which an app moves with the photo.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

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
                Some(n) => numbered(&file, n),
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

/// A file's name with `-number` after its stem: `DSC0001-2.ARW`.
fn numbered(file: &Path, number: u32) -> std::ffi::OsString {
    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
    match file.extension() {
        Some(ext) => format!("{stem}-{number}.{}", ext.to_string_lossy()).into(),
        None => format!("{stem}-{number}").into(),
    }
}

/// A photo to rename in its own folder (record 0017): each of its files, from its name now to its
/// new one.
#[derive(Debug, Clone)]
pub struct Renaming {
    pub photo: PathBuf,
    pub companion: Option<PathBuf>,
    files: Vec<(PathBuf, PathBuf)>,
}

impl Renaming {
    /// The photo's files, as `trashing::files_of` lists them, each named `stem` before its own
    /// extension, in its own folder: `DSC0001.ARW`, `.JPG` and `.xmp` as `Paris_001.ARW`, `.JPG`
    /// and `.xmp`.
    pub fn to_stem(photo: &Path, companion: Option<&Path>, stem: &str) -> Self {
        let files = trashing::files_of(photo, companion)
            .into_iter()
            .map(|file| {
                let name = match file.extension() {
                    Some(ext) => format!("{stem}.{}", ext.to_string_lossy()),
                    None => stem.to_owned(),
                };
                let to = file.with_file_name(name);
                (file, to)
            })
            .collect();
        Self { photo: photo.to_path_buf(), companion: companion.map(Path::to_path_buf), files }
    }

    /// Undo: a photo `rename` renamed, named back.
    pub fn back(placed: &Placed) -> Self {
        let files = placed.files.iter().map(|(from, to)| (to.clone(), from.clone())).collect();
        Self { photo: placed.to.clone(), companion: placed.companion.clone(), files }
    }

    /// The photo's new path.
    pub fn to(&self) -> &Path {
        &self.files[0].1
    }

    /// Each file, from its name now to its new one.
    pub fn files(&self) -> &[(PathBuf, PathBuf)] {
        &self.files
    }

    /// Whether every file keeps its name, in its case too.
    pub fn unchanged(&self) -> bool {
        self.files.iter().all(|(from, to)| from == to)
    }

    /// Whether the new name is one both systems allow and the listing shows, its files' names
    /// within 255 bytes.
    pub fn fault(&self) -> Option<NameFault> {
        let stem = self.to().file_stem().unwrap_or_default().to_string_lossy();
        name_fault(&stem).or_else(|| self.files.iter().any(|(_, to)| to.file_name().map_or(0, |n| n.len()) > MAX_NAME_BYTES).then_some(NameFault::TooLong))
    }

    fn placed(&self) -> Placed {
        Placed {
            photo: self.photo.clone(),
            to: self.to().to_path_buf(),
            companion: self.companion.as_ref().and_then(|c| self.files.iter().find(|(from, _)| from == c)).map(|(_, to)| to.clone()),
            files: self.files.clone(),
            done: Done::Moved,
        }
    }
}

/// The longest name, in bytes, APFS takes; NTFS takes 255 UTF-16 units, never fewer.
const MAX_NAME_BYTES: usize = 255;

/// What makes a name one that Windows or a Mac refuses, or that hides the photo (record 0017).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NameFault {
    /// No name, or spaces alone.
    Empty,
    /// A character Windows refuses in a name, a Mac's colon among them: `< > : " / \ | ? *`, and
    /// those from 0 to 31.
    Character { character: String },
    /// A space or a period at the end, which Windows' own Explorer doesn't allow.
    Ending,
    /// A period first, which hides the file; the listing leaves it out.
    Hidden,
    /// A name Windows keeps for a device, as CON or COM1, alone or before a period.
    Reserved { name: String },
    /// Longer than 255 bytes, with its extension.
    TooLong,
}

/// The names Windows keeps for devices, with the superscript digits it reads as digits
/// (Microsoft's Naming Files, Paths, and Namespaces).
const RESERVED: [&str; 28] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "COM\u{b9}", "COM\u{b2}", "COM\u{b3}", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
    "LPT7", "LPT8", "LPT9", "LPT\u{b9}", "LPT\u{b2}", "LPT\u{b3}",
];

/// Whether `stem`, a photo's name before its extension, is one both systems allow and the listing
/// shows (record 0017).
pub fn name_fault(stem: &str) -> Option<NameFault> {
    if stem.trim().is_empty() {
        return Some(NameFault::Empty);
    }
    if let Some(c) = stem.chars().find(|&c| "<>:\"/\\|?*".contains(c) || (c as u32) < 32) {
        return Some(NameFault::Character { character: c.to_string() });
    }
    if stem.starts_with('.') {
        return Some(NameFault::Hidden);
    }
    if stem.ends_with([' ', '.']) {
        return Some(NameFault::Ending);
    }
    let base = stem.split('.').next().unwrap_or_default().trim_end().to_uppercase();
    if RESERVED.contains(&base.as_str()) {
        return Some(NameFault::Reserved { name: base });
    }
    (stem.len() > MAX_NAME_BYTES).then_some(NameFault::TooLong)
}

/// A path as a drive that ignores case sees it: to know which photo of a run holds a name now.
fn folded(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

/// Whether `to` is free for `from`: no file has it, or the file there is `from` itself, its name in
/// another case on a drive that ignores case.
fn free_for(from: &Path, to: &Path) -> bool {
    match to.try_exists() {
        Ok(false) => true,
        Ok(true) => same_file::is_same_file(from, to).unwrap_or(false),
        Err(_) => false,
    }
}

/// A name before its extension, as a drive that ignores case sees it.
fn folded_stem(path: &Path) -> String {
    path.file_stem().unwrap_or_default().to_string_lossy().to_lowercase()
}

/// For each photo of a run, a file outside the run that has its new name, whatever its extension:
/// `Paris_001.jpg` for a RAW file renamed `Paris_001`, which would take that JPEG, or a sidecar of
/// that name, as its own. A file another photo of the run has now isn't one, as that photo will
/// leave its name. Each folder is listed once.
fn taken_by(list: &[Renaming]) -> Vec<Option<PathBuf>> {
    let held: HashSet<String> = list.iter().flat_map(|r| r.files.iter().map(|(from, _)| folded(from))).collect();
    let mut folders: HashMap<PathBuf, HashMap<String, Vec<PathBuf>>> = HashMap::new();
    list.iter()
        .map(|r| {
            let folder = r.to().parent().unwrap_or(Path::new("")).to_path_buf();
            let stems = folders.entry(folder.clone()).or_insert_with(|| {
                let mut stems: HashMap<String, Vec<PathBuf>> = HashMap::new();
                for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
                    stems.entry(folded_stem(&entry.path())).or_default().push(entry.path());
                }
                stems
            });
            stems.get(&folded_stem(r.to())).and_then(|files| files.iter().find(|f| !held.contains(&folded(f))).cloned())
        })
        .collect()
}

/// For each photo of a run, whether a file outside the run has its new name, whatever its
/// extension: for an app's preview, and its Keep Both.
pub fn taken(list: &[Renaming]) -> Vec<bool> {
    taken_by(list).iter().map(Option::is_some).collect()
}

/// Keep Both: each photo of a run whose new name a file outside the run has gets it with the
/// smallest number from 2 free in its folder and in the run, `Paris-2`, its JPEG and sidecar
/// alike. Says which were numbered.
pub fn number_taken(list: &mut [Renaming]) -> Vec<usize> {
    let taken = taken(list);
    let held: HashSet<String> = list.iter().flat_map(|r| r.files.iter().map(|(from, _)| folded(from))).collect();
    let in_run = |r: &Renaming| folded(&r.to().with_extension(""));
    let mut names: HashSet<String> = list.iter().map(in_run).collect();
    let mut numbered_ones = Vec::new();
    for i in (0..list.len()).filter(|&i| taken[i]) {
        let folder = list[i].to().parent().unwrap_or(Path::new("")).to_path_buf();
        let others: HashSet<String> = std::fs::read_dir(&folder).into_iter().flatten().flatten().filter(|e| !held.contains(&folded(&e.path()))).map(|e| folded_stem(&e.path())).collect();
        let free = (2..10_000).map(|n| list[i].files.iter().map(|(from, to)| (from.clone(), to.with_file_name(numbered(to, n)))).collect::<Vec<_>>()).find(|files| {
            let stem = folded_stem(&files[0].1);
            !others.contains(&stem) && !names.contains(&folded(&files[0].1.with_extension(""))) && files.iter().all(|(from, to)| free_for(from, to))
        });
        if let Some(files) = free {
            list[i].files = files;
            names.insert(in_run(&list[i]));
            numbered_ones.push(i);
        }
    }
    numbered_ones
}

/// Why a file's new name can't be had.
fn taken_cause(to: &Path) -> String {
    let folder = to.parent().map_or_else(String::new, |p| p.display().to_string());
    format!("a file named {} is already in {folder}", name_of(to))
}

/// Renames the photos of a run in their own folders, each with its JPEG and sidecar, never over a
/// file (record 0017). A photo goes once its new names are free; one whose new names another photo
/// of the run has now waits for it; when every photo left waits on another, one moves aside to
/// hidden names, freeing its own, and goes on from there once its new names are free. A name a file
/// outside the run has is never taken: that photo keeps its names. `each` hears of every photo, in
/// the order they go.
pub fn rename(list: &[Renaming], each: &mut dyn FnMut(usize, Result<Placed, Problem>)) {
    static ASIDE: AtomicU64 = AtomicU64::new(0);
    let failed = |i: usize, cause: String| Problem::NotRenamed { photo: name_of(&list[i].photo), cause };
    // Where each photo's files are now: their own names, or hidden ones once moved aside.
    let mut now: Vec<Vec<PathBuf>> = list.iter().map(|r| r.files.iter().map(|(from, _)| from.clone()).collect()).collect();
    // Which photo of the run has each name now.
    let mut holder: HashMap<String, usize> = HashMap::new();
    for (i, files) in now.iter().enumerate() {
        for file in files {
            holder.insert(folded(file), i);
        }
    }
    let taken = taken_by(list);
    let mut pending = Vec::new();
    for (i, renaming) in list.iter().enumerate() {
        if renaming.unchanged() {
            each(i, Ok(renaming.placed()));
        } else if let Some(file) = &taken[i] {
            each(i, Err(failed(i, taken_cause(file))));
        } else {
            pending.push(i);
        }
    }
    let mut aside = vec![false; list.len()];
    while !pending.is_empty() {
        let waiting: HashSet<usize> = pending.iter().copied().collect();
        let waits = |i: usize, holder: &HashMap<String, usize>| list[i].files.iter().any(|(_, to)| holder.get(&folded(to)).is_some_and(|h| *h != i && waiting.contains(h)));
        let ready: Vec<usize> = pending.iter().copied().filter(|&i| !waits(i, &holder)).collect();
        if ready.is_empty() {
            // Every photo left waits on another, as photos swapping names do: one moves aside.
            let Some(i) = pending.iter().copied().find(|&i| !aside[i]) else {
                for i in std::mem::take(&mut pending) {
                    each(i, Err(failed(i, "the photos' names wait on each other".into())));
                }
                break;
            };
            let hidden: Vec<(PathBuf, PathBuf)> = now[i]
                .iter()
                .map(|file| {
                    let n = ASIDE.fetch_add(1, Ordering::Relaxed);
                    let ext = file.extension().map_or_else(String::new, |e| format!(".{}", e.to_string_lossy()));
                    (file.clone(), file.with_file_name(format!(".photomatch-{}-{n}{ext}", std::process::id())))
                })
                .collect();
            match rename_all(&hidden) {
                Ok(()) => {
                    for (file, _) in &hidden {
                        holder.remove(&folded(file));
                    }
                    now[i] = hidden.into_iter().map(|(_, h)| h).collect();
                    aside[i] = true;
                }
                Err(error) => {
                    each(i, Err(failed(i, error.to_string())));
                    pending.retain(|&p| p != i);
                }
            }
            continue;
        }
        for i in ready {
            pending.retain(|&p| p != i);
            let files: Vec<(PathBuf, PathBuf)> = now[i].iter().cloned().zip(list[i].files.iter().map(|(_, to)| to.clone())).collect();
            let result = match files.iter().find(|(from, to)| !free_for(from, to)) {
                Some((_, to)) => Err(taken_cause(to)),
                None => rename_all(&files).map_err(|e| e.to_string()),
            };
            match result {
                Ok(()) => {
                    for file in &now[i] {
                        if holder.get(&folded(file)) == Some(&i) {
                            holder.remove(&folded(file));
                        }
                    }
                    for (_, to) in &files {
                        holder.insert(folded(to), i);
                    }
                    each(i, Ok(list[i].placed()));
                }
                Err(cause) if aside[i] => {
                    let own: Vec<PathBuf> = list[i].files.iter().map(|(from, _)| from.clone()).collect();
                    let back = come_back(&now[i], &own);
                    each(i, Err(failed(i, format!("{cause}; {back}"))));
                }
                Err(cause) => each(i, Err(failed(i, cause))),
            }
        }
    }
}

/// A photo moved aside whose new names couldn't be had: back to its own names, or, if another photo
/// took them meanwhile, to its own with the smallest number free. Says where it is.
fn come_back(hidden: &[PathBuf], own: &[PathBuf]) -> String {
    let back: Vec<(PathBuf, PathBuf)> = hidden.iter().cloned().zip(own.iter().cloned()).collect();
    if back.iter().all(|(from, to)| free_for(from, to)) && rename_all(&back).is_ok() {
        return "it keeps its name".into();
    }
    for n in 2..1000 {
        let numbered: Vec<(PathBuf, PathBuf)> = hidden.iter().cloned().zip(own.iter().map(|o| o.with_file_name(numbered(o, n)))).collect();
        if numbered.iter().all(|(from, to)| free_for(from, to)) && rename_all(&numbered).is_ok() {
            return format!("it's now named {}", name_of(&numbered[0].1));
        }
    }
    format!("it's now named {}, hidden", name_of(&hidden[0]))
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
        // A change of case alone is the same file, and goes (record 0017).
        if to.exists() && !same_file::is_same_file(from, to).unwrap_or(false) {
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
            // A change of case alone on a drive that ignores case: the same file (record 0017).
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && same_file::is_same_file(from, to).unwrap_or(false) => std::fs::rename(from, to),
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

    #[test]
    fn a_photo_is_renamed_with_its_jpeg_and_sidecar_and_named_back() {
        let dir = folder("rename");
        let (raw, jpeg) = shoot(&dir, "DSC0001");
        let renaming = Renaming::to_stem(&raw, Some(&jpeg), "Paris_001");
        assert_eq!(renaming.fault(), None);
        let mut placed = Vec::new();
        rename(&[renaming], &mut |_, result| placed.push(result.unwrap()));
        assert_eq!(names(&dir), ["Paris_001.ARW", "Paris_001.JPG", "Paris_001.xmp"]);
        assert_eq!(std::fs::read_to_string(dir.join("Paris_001.xmp")).unwrap(), "DSC0001 sidecar");
        assert_eq!(placed[0].companion.as_deref(), Some(dir.join("Paris_001.JPG").as_path()));
        rename(&[Renaming::back(&placed[0])], &mut |_, result| {
            result.unwrap();
        });
        assert_eq!(names(&dir), ["DSC0001.ARW", "DSC0001.JPG", "DSC0001.xmp"], "named back");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_change_of_case_alone_is_a_rename() {
        let dir = folder("rename-case");
        let (raw, jpeg) = shoot(&dir, "dsc0001");
        let renaming = Renaming::to_stem(&raw, Some(&jpeg), "DSC0001");
        assert_eq!(taken(std::slice::from_ref(&renaming)), [false], "its own name in another case isn't taken");
        let mut went = 0;
        rename(&[renaming], &mut |_, result| {
            result.unwrap();
            went += 1;
        });
        assert_eq!((went, names(&dir)), (1, vec!["DSC0001.ARW".to_string(), "DSC0001.JPG".into(), "DSC0001.xmp".into()]));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn photos_swap_names_and_a_sequence_moves_one_along() {
        let dir = folder("rename-swap");
        let (a, b, c) = (shoot(&dir, "A"), shoot(&dir, "B"), shoot(&dir, "C"));
        // A to B, B to C and C to A: each name another's.
        let run = [Renaming::to_stem(&a.0, Some(&a.1), "B"), Renaming::to_stem(&b.0, Some(&b.1), "C"), Renaming::to_stem(&c.0, Some(&c.1), "A")];
        let mut placed: Vec<Option<Placed>> = vec![None, None, None];
        rename(&run, &mut |i, result| placed[i] = Some(result.unwrap()));
        let all = ["A.ARW", "A.JPG", "A.xmp", "B.ARW", "B.JPG", "B.xmp", "C.ARW", "C.JPG", "C.xmp"];
        assert_eq!(names(&dir), all, "no hidden name left");
        assert_eq!(std::fs::read_to_string(dir.join("B.ARW")).unwrap(), "A raw");
        assert_eq!(std::fs::read_to_string(dir.join("C.JPG")).unwrap(), "B jpeg");
        assert_eq!(std::fs::read_to_string(dir.join("A.xmp")).unwrap(), "C sidecar");
        let back: Vec<Renaming> = placed.iter().map(|p| Renaming::back(p.as_ref().unwrap())).collect();
        rename(&back, &mut |_, result| {
            result.unwrap();
        });
        assert_eq!(names(&dir), all);
        assert_eq!(std::fs::read_to_string(dir.join("A.ARW")).unwrap(), "A raw", "named back");

        // S_001 to S_003 as S_002 to S_004: each name freed by the photo after it.
        let sequence = folder("rename-sequence");
        let photos: Vec<(PathBuf, PathBuf)> = (1..=3).map(|n| shoot(&sequence, &format!("S_{n:03}"))).collect();
        let run: Vec<Renaming> = photos.iter().enumerate().map(|(i, (raw, jpeg))| Renaming::to_stem(raw, Some(jpeg), &format!("S_{:03}", i + 2))).collect();
        let mut went = 0;
        rename(&run, &mut |_, result| {
            result.unwrap();
            went += 1;
        });
        assert_eq!(went, 3);
        assert_eq!(std::fs::read_to_string(sequence.join("S_004.ARW")).unwrap(), "S_003 raw");
        assert_eq!(std::fs::read_to_string(sequence.join("S_002.xmp")).unwrap(), "S_001 sidecar");
        assert!(!sequence.join("S_001.ARW").exists());
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(sequence);
    }

    #[test]
    fn a_name_another_file_has_is_never_taken() {
        let dir = folder("rename-taken");
        let (raw, jpeg) = shoot(&dir, "DSC0001");
        std::fs::write(dir.join("Paris.JPG"), "another photo").unwrap();
        let renaming = Renaming::to_stem(&raw, Some(&jpeg), "Paris");
        assert_eq!(taken(std::slice::from_ref(&renaming)), [true]);
        std::fs::write(dir.join("Lone.jpg"), "a photo alone").unwrap();
        assert_eq!(taken(&[Renaming::to_stem(&raw, None, "lone")]), [true], "a JPEG of that name, which the RAW file would take as its own");
        let mut problem = None;
        rename(&[renaming], &mut |_, result| problem = result.err());
        assert!(matches!(problem, Some(Problem::NotRenamed { .. })), "{problem:?}");
        assert_eq!(std::fs::read_to_string(dir.join("Paris.JPG")).unwrap(), "another photo", "never written over");
        assert_eq!(names(&dir), ["DSC0001.ARW", "DSC0001.JPG", "DSC0001.xmp", "Lone.jpg", "Paris.JPG"], "the photo keeps its names");

        // Keep Both: numbered, the number free in the run too.
        std::fs::write(dir.join("Paris-2.xmp"), "a sidecar alone").unwrap();
        let mut run = [Renaming::to_stem(&raw, Some(&jpeg), "Paris")];
        assert_eq!(number_taken(&mut run), [0]);
        rename(&run, &mut |_, result| {
            result.unwrap();
        });
        assert_eq!(names(&dir), ["Lone.jpg", "Paris-2.xmp", "Paris-3.ARW", "Paris-3.JPG", "Paris-3.xmp", "Paris.JPG"], "-2 taken by a sidecar alone, so -3");
        let (numbered_raw, numbered_jpeg) = (dir.join("Paris-3.ARW"), dir.join("Paris-3.JPG"));
        rename(&[Renaming::to_stem(&numbered_raw, Some(&numbered_jpeg), "DSC0001")], &mut |_, result| {
            result.unwrap();
        });
        std::fs::remove_file(dir.join("Paris-2.xmp")).unwrap();

        // Two photos of a run given one name: the second finds it taken by the first.
        let (other, _) = shoot(&dir, "DSC0002");
        let run = [Renaming::to_stem(&raw, Some(&jpeg), "Lyon"), Renaming::to_stem(&other, None, "Lyon")];
        let mut results = Vec::new();
        rename(&run, &mut |i, result| results.push((i, result.is_ok())));
        assert_eq!(results, [(0, true), (1, false)]);
        assert_eq!(std::fs::read_to_string(dir.join("Lyon.ARW")).unwrap(), "DSC0001 raw");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn names_both_systems_allow() {
        assert_eq!(name_fault("2026-11-24_001"), None);
        assert_eq!(name_fault("Paris. Day 2"), None);
        assert_eq!(name_fault("CONTACT"), None);
        assert_eq!(name_fault("  "), Some(NameFault::Empty));
        assert_eq!(name_fault("10:30"), Some(NameFault::Character { character: ":".into() }));
        assert_eq!(name_fault("Day 1/2"), Some(NameFault::Character { character: "/".into() }));
        assert_eq!(name_fault("Tab\there"), Some(NameFault::Character { character: "\t".into() }));
        assert_eq!(name_fault(".hidden"), Some(NameFault::Hidden));
        assert_eq!(name_fault("Paris."), Some(NameFault::Ending));
        assert_eq!(name_fault("Paris "), Some(NameFault::Ending));
        assert_eq!(name_fault("con"), Some(NameFault::Reserved { name: "CON".into() }));
        assert_eq!(name_fault("NUL.tar"), Some(NameFault::Reserved { name: "NUL".into() }));
        assert_eq!(name_fault("com\u{b9}"), Some(NameFault::Reserved { name: "COM\u{b9}".into() }));
        assert_eq!(name_fault(&"\u{e9}".repeat(128)), Some(NameFault::TooLong), "256 bytes");
        let dir = folder("rename-long");
        let (raw, _) = shoot(&dir, "DSC0001");
        assert_eq!(Renaming::to_stem(&raw, None, &"a".repeat(252)).fault(), Some(NameFault::TooLong), "with its extension");
        assert_eq!(Renaming::to_stem(&raw, None, &"a".repeat(251)).fault(), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
