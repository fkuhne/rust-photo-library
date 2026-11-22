//! The user's picks, stars and colour labels. They're kept in a file of their own, in the
//! app's data folder, until the store of plan step 1.3 takes them over; never in the photos
//! (record 0004). A RAW file's rating also goes into an XMP sidecar beside it, for Lightroom and
//! Capture One, and comes back from it when another app changed it (record 0014).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::files::write_durably;
use crate::problem::Problem;
use crate::sidecar::{self, SidecarError, Stamp};

/// Lightroom's three flag states, as the owner named them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pick {
    #[default]
    Unpicked,
    Picked,
    Rejected,
}

/// Lightroom's colour labels that have a key (6 to 9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Label {
    #[default]
    None,
    Red,
    Yellow,
    Green,
    Blue,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "StoredRating")]
pub struct Rating {
    pub pick: Pick,
    /// 0 to 5.
    pub stars: u8,
    pub label: Label,
}

/// A rating as the file holds it. Step 2.4's file said `"picked": true` instead of `"pick"`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredRating {
    #[serde(default)]
    pick: Option<Pick>,
    #[serde(default)]
    picked: bool,
    #[serde(default)]
    stars: u8,
    #[serde(default)]
    label: Label,
}

impl From<StoredRating> for Rating {
    fn from(stored: StoredRating) -> Self {
        let pick = stored.pick.unwrap_or(if stored.picked { Pick::Picked } else { Pick::Unpicked });
        Self { pick, stars: stored.stars, label: stored.label }
    }
}

/// A change to some photos' ratings. What's `None` stays as it is.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatingChange {
    pub pick: Option<Pick>,
    pub stars: Option<u8>,
    pub label: Option<Label>,
}

/// Some photos' ratings before a change and after it, in the same order, for undo.
#[derive(Debug, Serialize)]
pub struct Changed {
    pub before: Vec<Rating>,
    pub after: Vec<Rating>,
}

/// What the file keeps for a photo.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(flatten)]
    rating: Rating,
    /// When the rating last changed in PhotoMatch, or was taken from the sidecar, in
    /// milliseconds since 1970; 0 for a rating from before step 2.16.
    #[serde(default)]
    changed_ms: u64,
    /// The sidecar's stamp, as it was after PhotoMatch last wrote or read it.
    #[serde(default)]
    sidecar: Option<Stamp>,
    /// The rating as PhotoMatch last wrote it to the sidecar or read it from there; `None` before
    /// step 2.21. Where it differs from `rating`, a write is owed, as after one that failed.
    #[serde(default)]
    synced: Option<Rating>,
    /// The photographer's tags, each its levels joined by " > ", sorted (record 0011); left out of
    /// the file when there are none, so a store from before reads as it was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
}

impl Entry {
    /// A photo with no rating isn't kept, unless its sidecar's stamp must be.
    fn worth_keeping(&self) -> bool {
        self.rating != Rating::default() || self.sidecar.is_some() || !self.tags.is_empty()
    }

    /// Taken from a sidecar another app changed, the photo's tags kept: the sidecar doesn't hold
    /// them yet (record 0011).
    fn from_sidecar(rating: Rating, stamp: Stamp, tags: Vec<String>) -> Self {
        Self { rating, changed_ms: stamp.modified_ms, sidecar: Some(stamp), synced: Some(rating), tags }
    }

    /// Whether the sidecar has another app's change that PhotoMatch hasn't seen, newer than
    /// PhotoMatch's own last change.
    fn behind(&self, stamp: Stamp) -> bool {
        self.sidecar != Some(stamp) && stamp.modified_ms > self.changed_ms
    }

    /// Whether PhotoMatch's rating has yet to reach the sidecar.
    fn owes_a_write(&self) -> bool {
        self.synced.is_some_and(|synced| synced != self.rating)
    }
}

/// Counts as a write under way while it lives, so that quitting waits for it.
struct Writing<'a>(&'a Ratings);

impl<'a> Writing<'a> {
    fn new(ratings: &'a Ratings) -> Self {
        ratings.start_writing();
        Self(ratings)
    }
}

impl Drop for Writing<'_> {
    fn drop(&mut self) {
        self.0.end_writing();
    }
}

/// What reading the sidecars of a folder did.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Synced {
    /// The photos whose rating came from a sidecar changed by another app, by their place in
    /// the photos asked about.
    pub taken: Vec<(usize, Rating)>,
    pub read: usize,
    pub written: usize,
}

/// A change of the photographer's tags (record 0011).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TagChange {
    /// These tags added, each as `tags_from` reads it.
    Add { tags: Vec<String> },
    /// These tags taken off, with their levels below.
    Remove { tags: Vec<String> },
    /// One tag renamed, its levels below with it.
    Rename { from: String, to: String },
}

/// Each photo's tags before and after a change, for undo.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TagsChanged {
    pub before: Vec<Vec<String>>,
    pub after: Vec<Vec<String>>,
}

/// The tags typed in `text`, as Lightroom Classic has them typed: commas between tags, `>` or
/// `|` between levels; each level trimmed, empty ones dropped (record 0011).
pub fn tags_from(text: &str) -> Vec<String> {
    tidy(text.split(',').filter_map(tag_from).collect())
}

/// One tag typed: its levels, trimmed, joined by " > "; `None` if it has none.
pub fn tag_from(text: &str) -> Option<String> {
    let levels: Vec<&str> = text.split(['>', '|']).map(str::trim).filter(|level| !level.is_empty()).collect();
    (!levels.is_empty()).then(|| levels.join(" > "))
}

/// A tag's levels.
fn levels(tag: &str) -> Vec<&str> {
    tag.split(" > ").collect()
}

/// Whether `tag` is `parent`, or one of its levels below, letters' case aside.
fn under(tag: &str, parent: &str) -> bool {
    let (tag, parent) = (levels(tag), levels(parent));
    tag.len() >= parent.len() && tag.iter().zip(&parent).all(|(a, b)| a.to_lowercase() == b.to_lowercase())
}

/// Sorted, letters' case aside, without repeats; the first spelling kept.
fn tidy(tags: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut tags: Vec<String> = tags.into_iter().filter(|tag| seen.insert(tag.to_lowercase())).collect();
    tags.sort_by_key(|tag| tag.to_lowercase());
    tags
}

/// A photo's tags after `change`.
fn retagged(change: &TagChange, tags: &[String]) -> Vec<String> {
    match change {
        TagChange::Add { tags: new } => tags.iter().cloned().chain(new.iter().filter_map(|tag| tag_from(tag))).collect(),
        TagChange::Remove { tags: gone } => {
            let gone: Vec<String> = gone.iter().filter_map(|tag| tag_from(tag)).collect();
            tags.iter().filter(|tag| !gone.iter().any(|g| under(tag, g))).cloned().collect()
        }
        TagChange::Rename { from, to } => {
            let (Some(from), Some(to)) = (tag_from(from), tag_from(to)) else { return tags.to_vec() };
            let depth = levels(&from).len();
            tags.iter()
                .map(|tag| if under(tag, &from) { std::iter::once(to.as_str()).chain(levels(tag)[depth..].iter().copied()).collect::<Vec<_>>().join(" > ") } else { tag.clone() })
                .collect()
        }
    }
}

pub struct Ratings {
    file: PathBuf,
    /// Set when the file couldn't be opened at launch, nor kept aside: the ratings are kept in
    /// memory only, and the file is never written over (plan step 2.35).
    memory_only: bool,
    /// By the photo's path.
    entries: Mutex<HashMap<PathBuf, Entry>>,
    /// Held while a sidecar is read or written, so that two never cross.
    sidecar_io: Mutex<()>,
    /// Sidecar writes queued or under way, and a signal when one ends.
    writing: Mutex<usize>,
    written: Condvar,
    /// The folders whose sidecars couldn't be read, or written, each said once, and what's yet
    /// to be said, with what went wrong with the ratings file at launch.
    problems: Mutex<(HashSet<(PathBuf, bool)>, Vec<Problem>)>,
    sidecars_written: AtomicUsize,
    sidecars_read: AtomicUsize,
}

impl Ratings {
    /// The ratings kept in `file`, or none if it doesn't exist yet. The app always starts, and
    /// says what went wrong (plan step 2.35): a file that can't be understood, cut short by a
    /// crash say, is kept aside under another name, and the app starts without it; one that
    /// can't be opened, nor kept aside, leaves the ratings in memory only, never written over it.
    pub fn open(file: PathBuf) -> Arc<Self> {
        let mut said = Vec::new();
        let mut unreadable = None;
        let entries = match std::fs::read(&file) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(entries) => entries,
                Err(error) => {
                    let aside = file.with_extension(format!("damaged-{}.json", now_ms()));
                    match std::fs::rename(&file, &aside) {
                        Ok(()) => said.push(Problem::RatingsFileDamaged { kept_as: aside.display().to_string(), cause: error.to_string() }),
                        Err(e) => unreadable = Some(format!("can't understand it ({error}), nor keep it aside: {e}")),
                    }
                    HashMap::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => {
                unreadable = Some(format!("can't read it: {e}"));
                HashMap::new()
            }
        };
        let memory_only = unreadable.is_some();
        if let Some(cause) = unreadable {
            said.push(Problem::RatingsFileUnreadable { file: file.display().to_string(), cause });
        }
        if let Some(problem) = said.last() {
            eprintln!("photomatch: the ratings file: {problem}");
        }
        Arc::new(Self {
            file,
            memory_only,
            entries: Mutex::new(entries),
            sidecar_io: Mutex::new(()),
            writing: Mutex::new(0),
            written: Condvar::new(),
            problems: Mutex::new((HashSet::new(), said)),
            sidecars_written: AtomicUsize::new(0),
            sidecars_read: AtomicUsize::new(0),
        })
    }

    pub fn get(&self, photo: &Path) -> Rating {
        self.entries.lock().ok().and_then(|m| m.get(photo).map(|e| e.rating)).unwrap_or_default()
    }

    /// The photographer's tags on `photo`, sorted (record 0011).
    pub fn tags(&self, photo: &Path) -> Vec<String> {
        self.entries.lock().ok().and_then(|m| m.get(photo).map(|e| e.tags.clone())).unwrap_or_default()
    }

    /// Applies `change` to the tags of `photos`, saves, and returns their tags before and after.
    /// No sidecar is written: they don't hold tags yet (record 0011). If saving fails, nothing
    /// changes.
    pub fn retag(&self, photos: &[PathBuf], change: &TagChange) -> Result<TagsChanged, Problem> {
        self.update_tags(photos, |_, tags| retagged(change, tags))
    }

    /// Gives each of `photos` the tags at the same place in `tags`, as undo puts them back;
    /// saves, and returns them.
    pub fn set_tags(&self, photos: &[PathBuf], tags: &[Vec<String>]) -> Result<Vec<Vec<String>>, Problem> {
        if photos.len() != tags.len() {
            return Err(format!("{} photos but {} sets of tags", photos.len(), tags.len()).into());
        }
        Ok(self.update_tags(photos, |i, _| tags[i].clone())?.after)
    }

    fn update_tags(&self, photos: &[PathBuf], retag: impl Fn(usize, &[String]) -> Vec<String>) -> Result<TagsChanged, Problem> {
        let mut entries = self.entries.lock().map_err(|e| e.to_string())?;
        let mut previous = Vec::new();
        let (mut before, mut after) = (Vec::new(), Vec::new());
        for (i, photo) in photos.iter().enumerate() {
            let old = entries.get(photo).cloned();
            let mut entry = old.clone().unwrap_or_default();
            previous.push((photo.clone(), old));
            let was = std::mem::take(&mut entry.tags);
            entry.tags = tidy(retag(i, &was));
            before.push(was);
            after.push(entry.tags.clone());
            if entry.worth_keeping() {
                entries.insert(photo.clone(), entry);
            } else {
                entries.remove(photo);
            }
        }
        if let Err(error) = self.save(&entries) {
            for (photo, entry) in previous {
                match entry {
                    Some(entry) => entries.insert(photo, entry),
                    None => entries.remove(&photo),
                };
            }
            return Err(error);
        }
        Ok(TagsChanged { before, after })
    }

    /// Applies `change` to `photos`, saves, and returns their ratings before and after. Their
    /// sidecars are written right after, in the background.
    pub fn change(self: &Arc<Self>, photos: &[PathBuf], change: RatingChange) -> Result<Changed, Problem> {
        self.update(photos, |_, mut rating| {
            if let Some(pick) = change.pick {
                rating.pick = pick;
            }
            if let Some(stars) = change.stars {
                rating.stars = stars;
            }
            if let Some(label) = change.label {
                rating.label = label;
            }
            rating
        })
    }

    /// Gives each of `photos` the rating at the same place in `ratings`, as undo puts them back;
    /// saves, and returns them.
    pub fn set(self: &Arc<Self>, photos: &[PathBuf], ratings: &[Rating]) -> Result<Vec<Rating>, Problem> {
        if photos.len() != ratings.len() {
            return Err(format!("{} photos but {} ratings", photos.len(), ratings.len()).into());
        }
        Ok(self.update(photos, |i, _| ratings[i])?.after)
    }

    /// Gives each photo the rating `rate` makes from its place and its rating now; saves, and
    /// has the sidecars of those that changed written. A sidecar another app changed since the
    /// last sync is read first, so a key changes only what it says, on top of that app's change.
    /// If saving fails, nothing changes.
    fn update(self: &Arc<Self>, photos: &[PathBuf], rate: impl Fn(usize, Rating) -> Rating) -> Result<Changed, Problem> {
        // The sidecars, then the entries: the order the writes take them in too.
        let io = self.sidecar_io.lock().map_err(|e| e.to_string())?;
        let theirs = self.newer_sidecars(photos);
        let mut entries = self.entries.lock().map_err(|e| e.to_string())?;
        let now = now_ms();
        let mut changed = Vec::new();
        let mut previous = Vec::new();
        let (before, after) = photos
            .iter()
            .enumerate()
            .map(|(i, photo)| {
                previous.push((photo.clone(), entries.get(photo).cloned()));
                let mut entry = match theirs.get(photo) {
                    Some(&(rating, stamp)) => Entry::from_sidecar(rating, stamp, entries.get(photo).map(|e| e.tags.clone()).unwrap_or_default()),
                    None => entries.get(photo).cloned().unwrap_or_default(),
                };
                let before = entry.rating;
                entry.rating = rate(i, before);
                entry.rating.stars = entry.rating.stars.min(5);
                if entry.rating != before {
                    entry.changed_ms = now;
                    changed.push(photo.clone());
                }
                let after = entry.rating;
                if entry.worth_keeping() {
                    entries.insert(photo.clone(), entry);
                } else {
                    entries.remove(photo);
                }
                (before, after)
            })
            .unzip();
        if let Err(error) = self.save(&entries) {
            for (photo, entry) in previous {
                match entry {
                    Some(entry) => entries.insert(photo, entry),
                    None => entries.remove(&photo),
                };
            }
            return Err(error);
        }
        drop(entries);
        drop(io);
        self.write_sidecars_soon(changed);
        Ok(Changed { before, after })
    }

    /// The ratings in the sidecars of `photos` that another app changed since PhotoMatch last
    /// wrote or read them, and later than PhotoMatch's own last change, with their stamps.
    fn newer_sidecars(&self, photos: &[PathBuf]) -> HashMap<PathBuf, (Rating, Stamp)> {
        let known: Vec<_> = match self.entries.lock() {
            Ok(entries) => photos.iter().filter(|p| sidecar::has_sidecar(p)).map(|p| (p.clone(), entries.get(p).cloned().unwrap_or_default())).collect(),
            Err(_) => return HashMap::new(),
        };
        let mut theirs = HashMap::new();
        for (photo, entry) in known {
            let path = sidecar::path_for(&photo);
            let Some(stamp) = sidecar::stamp(&path).filter(|&stamp| entry.behind(stamp)) else { continue };
            match sidecar::read(&path) {
                Ok(rating) => {
                    self.sidecars_read.fetch_add(1, Ordering::Relaxed);
                    theirs.insert(photo, (rating, stamp));
                }
                Err(error) => self.sidecar_problem(&photo, &error, false),
            }
        }
        theirs
    }

    /// Writes the sidecars of `photos` in the background, as they are when it gets to them.
    fn write_sidecars_soon(self: &Arc<Self>, photos: Vec<PathBuf>) {
        let photos: Vec<_> = photos.into_iter().filter(|p| sidecar::has_sidecar(p)).collect();
        if photos.is_empty() {
            return;
        }
        // Counted from now, before the thread starts, so that quitting can't miss it.
        self.start_writing();
        let this = self.clone();
        std::thread::spawn(move || {
            this.write_sidecars(&photos);
            this.end_writing();
        });
    }

    fn start_writing(&self) {
        if let Ok(mut writing) = self.writing.lock() {
            *writing += 1;
        }
    }

    fn end_writing(&self) {
        if let Ok(mut writing) = self.writing.lock() {
            *writing -= 1;
        }
        self.written.notify_all();
    }

    /// Writes each photo's rating into its sidecar, and keeps the sidecars' new stamps. A
    /// sidecar another app changed since, later than PhotoMatch's change, is left for the next
    /// sync to read. Returns how many it wrote.
    fn write_sidecars(&self, photos: &[PathBuf]) -> usize {
        let mut written = 0;
        for photo in photos {
            let Ok(_io) = self.sidecar_io.lock() else { break };
            let path = sidecar::path_for(photo);
            let entry = self.entries.lock().ok().and_then(|m| m.get(photo).cloned()).unwrap_or_default();
            if sidecar::stamp(&path).is_some_and(|stamp| entry.behind(stamp)) {
                continue;
            }
            match sidecar::write(&path, entry.rating) {
                Ok(stamp) => {
                    written += 1;
                    if let Ok(mut entries) = self.entries.lock() {
                        // A key pressed meanwhile leaves the entry owing its own write.
                        let now = entries.entry(photo.clone()).or_default();
                        now.sidecar = Some(stamp);
                        now.synced = Some(entry.rating);
                    }
                }
                Err(error) => self.sidecar_problem(photo, &error, true),
            }
        }
        self.sidecars_written.fetch_add(written, Ordering::Relaxed);
        if written > 0 {
            if let Ok(entries) = self.entries.lock() {
                if let Err(error) = self.save(&entries) {
                    eprintln!("photomatch: {error}");
                }
            }
        }
        written
    }

    /// Reads the sidecars of `photos` that another app changed since PhotoMatch last wrote or
    /// read them, and takes their ratings when they're newer than PhotoMatch's last change.
    /// Writes PhotoMatch's rating where it's newer, where a write is owed, or where a rated photo
    /// has no sidecar. Quitting waits for it.
    pub fn sync(&self, photos: &[PathBuf]) -> Synced {
        let _writing = Writing::new(self);
        let mut synced = Synced::default();
        let mut to_write = Vec::new();
        for (i, photo) in photos.iter().enumerate() {
            if !sidecar::has_sidecar(photo) {
                continue;
            }
            let Ok(_io) = self.sidecar_io.lock() else { break };
            let path = sidecar::path_for(photo);
            let entry = self.entries.lock().ok().and_then(|m| m.get(photo).cloned());
            match (sidecar::stamp(&path), entry) {
                (None, Some(entry)) if entry.rating != Rating::default() => to_write.push(photo.clone()),
                (None, _) => {}
                (Some(stamp), Some(entry)) if entry.sidecar == Some(stamp) => {
                    if entry.owes_a_write() {
                        to_write.push(photo.clone());
                    }
                }
                (Some(stamp), Some(entry)) if entry.changed_ms > stamp.modified_ms => to_write.push(photo.clone()),
                (Some(stamp), entry) => match sidecar::read(&path) {
                    Ok(rating) => {
                        synced.read += 1;
                        if let Ok(mut entries) = self.entries.lock() {
                            let tags = entries.get(photo).map(|e| e.tags.clone()).unwrap_or_default();
                            entries.insert(photo.clone(), Entry::from_sidecar(rating, stamp, tags));
                        }
                        if rating != entry.as_ref().map(|e| e.rating).unwrap_or_default() {
                            synced.taken.push((i, rating));
                        }
                    }
                    Err(error) => self.sidecar_problem(photo, &error, false),
                },
            }
        }
        self.sidecars_read.fetch_add(synced.read, Ordering::Relaxed);
        if synced.read > 0 {
            if let Ok(entries) = self.entries.lock() {
                if let Err(error) = self.save(&entries) {
                    eprintln!("photomatch: {error}");
                }
            }
        }
        synced.written = self.write_sidecars(&to_write);
        synced
    }

    /// Waits for the sidecars still being written, for at most `limit`, as the app quits.
    pub fn flush(&self, limit: Duration) {
        let Ok(writing) = self.writing.lock() else { return };
        let _ = self.written.wait_timeout_while(writing, limit, |writing| *writing > 0);
    }

    /// How many sidecars were written and read since the app started.
    pub fn sidecar_counts(&self) -> (usize, usize) {
        (self.sidecars_written.load(Ordering::Relaxed), self.sidecars_read.load(Ordering::Relaxed))
    }

    /// What went wrong since the last call: with the ratings file at launch, and with sidecars,
    /// once per folder and kind.
    pub fn take_problems(&self) -> Vec<Problem> {
        self.problems.lock().map(|mut p| std::mem::take(&mut p.1)).unwrap_or_default()
    }

    /// A sidecar that couldn't be read or written (plan step 2.35): one the disk refused to write
    /// means the folder's sidecars can't be kept up to date; one that can't be understood, or
    /// read, is left as it is.
    fn sidecar_problem(&self, photo: &Path, error: &SidecarError, writing: bool) {
        eprintln!("photomatch: sidecar: {error}");
        let folder = photo.parent().unwrap_or(Path::new("")).to_path_buf();
        let name = |p: &Path| p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let problem = match error {
            SidecarError::Disk(cause) if writing => Problem::SidecarsNotWritable { folder: name(&folder), cause: cause.clone() },
            _ => Problem::SidecarUnreadable { photo: name(photo), cause: error.to_string() },
        };
        let key = (folder, matches!(problem, Problem::SidecarsNotWritable { .. }));
        if let Ok(mut problems) = self.problems.lock() {
            if problems.0.insert(key) {
                problems.1.push(problem);
            }
        }
    }

    /// Writes the ratings file, unless it couldn't be opened at launch.
    fn save(&self, entries: &HashMap<PathBuf, Entry>) -> Result<(), Problem> {
        if self.memory_only {
            return Ok(());
        }
        let bytes = serde_json::to_vec_pretty(entries).map_err(|e| e.to_string())?;
        write_durably(&self.file, &bytes).map_err(|cause| Problem::RatingsNotSaved { cause })
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("photomatch-ratings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A folder with a RAW file, a DNG and a JPEG in it, and the ratings file beside them.
    fn library(name: &str) -> (PathBuf, Arc<Ratings>, [PathBuf; 3]) {
        let dir = temp_folder(name);
        let photos = ["DSC1.ARW", "L1.DNG", "IMG1.JPG"].map(|n| dir.join(n));
        for photo in &photos {
            std::fs::write(photo, b"a photo").unwrap();
        }
        (dir.clone(), Ratings::open(dir.join("ratings.json")), photos)
    }

    fn picked(stars: u8) -> RatingChange {
        RatingChange { pick: Some(Pick::Picked), stars: Some(stars), ..Default::default() }
    }

    #[test]
    fn changes_are_saved_and_read_back() {
        let dir = temp_folder("ratings");
        let file = dir.join("ratings.json");
        let (a, b) = (dir.join("a.ARW"), dir.join("b.NEF"));

        let ratings = Ratings::open(file.clone());
        ratings.change(&[a.clone(), b.clone()], RatingChange { pick: Some(Pick::Picked), ..Default::default() }).unwrap();
        ratings.change(&[a.clone()], RatingChange { stars: Some(9), label: Some(Label::Green), ..Default::default() }).unwrap();
        ratings.change(&[b.clone()], RatingChange { pick: Some(Pick::Unpicked), ..Default::default() }).unwrap();
        ratings.flush(Duration::from_secs(5));

        let reopened = Ratings::open(file);
        assert_eq!(reopened.get(&a), Rating { pick: Pick::Picked, stars: 5, label: Label::Green });
        assert_eq!(reopened.get(&b), Rating::default());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn step_2_4_picks_carry_over() {
        let dir = temp_folder("ratings-2-4");
        let file = dir.join("ratings.json");
        std::fs::write(&file, r#"{"/photos/a.ARW": {"picked": true, "stars": 3}, "/photos/b.ARW": {"picked": false, "stars": 2}}"#).unwrap();

        let ratings = Ratings::open(file);
        assert_eq!(ratings.get(Path::new("/photos/a.ARW")), Rating { pick: Pick::Picked, stars: 3, label: Label::None });
        assert_eq!(ratings.get(Path::new("/photos/b.ARW")), Rating { pick: Pick::Unpicked, stars: 2, label: Label::None });
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn set_puts_back_each_photos_own_rating() {
        let dir = temp_folder("ratings-set");
        let (a, b) = (dir.join("a.ARW"), dir.join("b.ARW"));
        let ratings = Ratings::open(dir.join("ratings.json"));
        ratings.change(&[a.clone()], RatingChange { stars: Some(4), ..Default::default() }).unwrap();
        let before = [ratings.get(&a), ratings.get(&b)];

        let changed = ratings.change(&[a.clone(), b.clone()], RatingChange { pick: Some(Pick::Rejected), ..Default::default() }).unwrap();
        assert_eq!(changed.before, before);
        let after = ratings.set(&[a.clone(), b.clone()], &before).unwrap();

        assert_eq!(after, before);
        assert_eq!(ratings.get(&a), Rating { pick: Pick::Unpicked, stars: 4, label: Label::None });
        assert!(ratings.set(&[a], &[]).is_err(), "a rating for each photo");
        ratings.flush(Duration::from_secs(5));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_raw_files_rating_reaches_its_sidecar_but_not_a_dngs_or_a_jpegs() {
        let (dir, ratings, [raw, dng, jpeg]) = library("sidecar-write");
        ratings.change(&[raw.clone(), dng.clone(), jpeg.clone()], picked(4)).unwrap();
        ratings.flush(Duration::from_secs(5));

        assert_eq!(sidecar::read(&dir.join("DSC1.xmp")).unwrap(), Rating { pick: Pick::Picked, stars: 4, label: Label::None });
        assert!(!dir.join("L1.xmp").exists() && !dir.join("IMG1.xmp").exists());
        assert_eq!(ratings.sidecar_counts().0, 1);
        assert!(ratings.sync(&[raw, dng, jpeg]).taken.is_empty(), "in step with the sidecar it wrote");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_sidecar_changed_by_another_app_since_wins() {
        let (dir, ratings, [raw, ..]) = library("sidecar-theirs");
        ratings.change(&[raw.clone()], picked(4)).unwrap();
        ratings.flush(Duration::from_secs(5));

        // Lightroom rejects it and gives it two stars, a moment later.
        std::thread::sleep(Duration::from_millis(20));
        sidecar::write(&dir.join("DSC1.xmp"), Rating { pick: Pick::Rejected, stars: 2, label: Label::Red }).unwrap();
        let synced = ratings.sync(&[raw.clone()]);
        let theirs = Rating { pick: Pick::Rejected, stars: 2, label: Label::Red };
        assert_eq!((synced.taken, synced.read, synced.written), (vec![(0, theirs)], 1, 0));
        assert_eq!(ratings.get(&raw), theirs);

        let again = ratings.sync(&[raw.clone()]);
        assert_eq!((again.read, again.written), (0, 0), "nothing to do once in step");
        assert_eq!(Ratings::open(dir.join("ratings.json")).get(&raw), theirs, "kept in the file");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn photomatchs_newer_change_wins_and_a_missing_sidecar_is_written() {
        let (dir, ratings, [raw, ..]) = library("sidecar-ours");
        let path = dir.join("DSC1.xmp");
        // Another app's sidecar from an hour ago, which PhotoMatch hasn't seen.
        sidecar::write(&path, Rating { stars: 1, ..Default::default() }).unwrap();
        let hour_ago = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(hour_ago).unwrap();
        // PhotoMatch's rating, given now, whose write didn't happen: as if it failed.
        ratings.entries.lock().unwrap().insert(raw.clone(), Entry { rating: Rating { stars: 5, ..Default::default() }, changed_ms: now_ms(), ..Default::default() });

        let synced = ratings.sync(&[raw.clone()]);
        assert_eq!((synced.taken.len(), synced.read, synced.written), (0, 0, 1));
        assert_eq!(sidecar::read(&path).unwrap().stars, 5);

        std::fs::remove_file(&path).unwrap();
        assert_eq!(ratings.sync(&[raw.clone()]).written, 1, "a rated photo gets its sidecar back");
        assert_eq!(sidecar::read(&path).unwrap().stars, 5);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_sidecar_wins_over_a_rating_from_before_sidecars() {
        let (dir, ratings, [raw, ..]) = library("sidecar-before");
        ratings.entries.lock().unwrap().insert(raw.clone(), Entry { rating: Rating { stars: 3, ..Default::default() }, ..Default::default() });
        sidecar::write(&dir.join("DSC1.xmp"), Rating { pick: Pick::Picked, ..Default::default() }).unwrap();

        let synced = ratings.sync(&[raw.clone()]);
        assert_eq!(synced.taken, vec![(0, Rating { pick: Pick::Picked, ..Default::default() })]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_sidecar_that_cant_be_written_is_said_once_per_folder() {
        let (dir, ratings, [raw, ..]) = library("sidecar-damaged");
        std::fs::write(dir.join("DSC1.xmp"), "not a sidecar").unwrap();
        ratings.change(&[raw.clone()], picked(2)).unwrap();
        ratings.flush(Duration::from_secs(5));
        ratings.sync(&[raw.clone()]);

        let problems = ratings.take_problems();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(matches!(&problems[0], Problem::SidecarUnreadable { photo, cause } if photo == "DSC1.ARW" && cause.contains("left as it is")), "{:?}", problems[0]);
        assert_eq!(std::fs::read_to_string(dir.join("DSC1.xmp")).unwrap(), "not a sidecar");
        assert_eq!(ratings.get(&raw).stars, 2, "the rating stays in PhotoMatch");
        std::fs::remove_dir_all(dir).unwrap();
    }

    // Plan step 2.21: the review's fixes 1, 2, 4, 5 and 6.

    #[test]
    fn a_key_changes_only_what_it_says_on_top_of_another_apps_change() {
        let (dir, ratings, [raw, ..]) = library("sidecar-merge");
        ratings.change(&[raw.clone()], picked(4)).unwrap();
        ratings.flush(Duration::from_secs(5));

        // Lightroom gives it two stars, and PhotoMatch hasn't synced since; then a red label here.
        std::thread::sleep(Duration::from_millis(20));
        let lightrooms = Rating { pick: Pick::Picked, stars: 2, label: Label::None };
        sidecar::write(&dir.join("DSC1.xmp"), lightrooms).unwrap();
        let changed = ratings.change(&[raw.clone()], RatingChange { label: Some(Label::Red), ..Default::default() }).unwrap();
        ratings.flush(Duration::from_secs(5));

        let both = Rating { label: Label::Red, ..lightrooms };
        assert_eq!((changed.before, changed.after), (vec![lightrooms], vec![both]), "undo goes back to Lightroom's");
        assert_eq!(sidecar::read(&dir.join("DSC1.xmp")).unwrap(), both, "Lightroom's two stars kept");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    fn set_writable(dir: &Path, writable: bool) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(if writable { 0o755 } else { 0o555 })).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_sidecar_write_is_tried_again_at_the_next_sync() {
        let dir = temp_folder("sidecar-retry");
        let raw = dir.join("DSC1.ARW");
        std::fs::write(&raw, b"a photo").unwrap();
        let data = temp_folder("sidecar-retry-data");
        let ratings = Ratings::open(data.join("ratings.json"));
        ratings.change(&[raw.clone()], picked(3)).unwrap();
        ratings.flush(Duration::from_secs(5));

        // The photos' folder turns read-only, as a card's lock might: the next write fails.
        set_writable(&dir, false);
        ratings.change(&[raw.clone()], RatingChange { stars: Some(5), ..Default::default() }).unwrap();
        ratings.flush(Duration::from_secs(5));
        set_writable(&dir, true);
        assert_eq!(sidecar::read(&dir.join("DSC1.xmp")).unwrap().stars, 3);

        assert_eq!(ratings.sync(&[raw.clone()]).written, 1, "owed, so written at the next sync");
        assert_eq!(sidecar::read(&dir.join("DSC1.xmp")).unwrap().stars, 5);
        assert_eq!(ratings.sync(&[raw]).written, 0, "and not again");
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::remove_dir_all(data).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_rating_that_cant_be_saved_changes_nothing() {
        let data = temp_folder("unsaved");
        // A JPEG, so no sidecar is written in the background while the folder changes.
        let photo = data.join("a.JPG");
        let ratings = Ratings::open(data.join("ratings.json"));
        ratings.change(&[photo.clone()], picked(1)).unwrap();

        set_writable(&data, false);
        let result = ratings.change(&[photo.clone()], picked(5));
        set_writable(&data, true);
        assert!(result.is_err());
        assert_eq!(ratings.get(&photo).stars, 1, "the rating the interface still shows");
        ratings.flush(Duration::from_secs(5));
        std::fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn a_ratings_file_that_cant_be_opened_leaves_the_ratings_in_memory_and_it_as_it_is() {
        // A folder where the file should be: reading it fails, and so would writing it.
        let dir = temp_folder("unreadable-file");
        let file = dir.join("ratings.json");
        std::fs::create_dir_all(file.join("inside")).unwrap();

        let ratings = Ratings::open(file.clone());
        let problems = ratings.take_problems();
        assert!(problems.len() == 1 && matches!(problems[0], Problem::RatingsFileUnreadable { .. }), "{problems:?}");
        let photo = dir.join("a.jpg");
        ratings.change(&[photo.clone()], RatingChange { stars: Some(3), ..Default::default() }).expect("rating still works, in memory");
        assert_eq!(ratings.get(&photo).stars, 3);
        assert!(file.join("inside").is_dir(), "the file is never written over");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_damaged_ratings_file_is_kept_aside_and_the_app_starts_without_it() {
        let dir = temp_folder("damaged-file");
        let file = dir.join("ratings.json");
        let cut_short = r#"{"/photos/a.ARW": {"pick": "pic"#;
        std::fs::write(&file, cut_short).unwrap();

        let ratings = Ratings::open(file.clone());
        assert_eq!(ratings.get(Path::new("/photos/a.ARW")), Rating::default());
        let problems = ratings.take_problems();
        assert!(problems.len() == 1 && matches!(problems[0], Problem::RatingsFileDamaged { .. }), "{problems:?}");
        let aside: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p != &file).collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(std::fs::read_to_string(&aside[0]).unwrap(), cut_short, "kept as it was");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn quitting_waits_for_the_writes_a_sync_makes() {
        let (dir, ratings, _) = library("sidecar-flush");
        // Rated RAW files with no sidecar yet: a sync writes them all.
        let photos: Vec<_> = (0..50).map(|i| dir.join(format!("P{i}.ARW"))).collect();
        for photo in &photos {
            std::fs::write(photo, b"a photo").unwrap();
            ratings.entries.lock().unwrap().insert(photo.clone(), Entry { rating: Rating { stars: 1, ..Default::default() }, changed_ms: now_ms(), ..Default::default() });
        }
        let syncing = {
            let (ratings, photos) = (ratings.clone(), photos.clone());
            std::thread::spawn(move || ratings.sync(&photos))
        };
        while *ratings.writing.lock().unwrap() == 0 && !syncing.is_finished() {
            std::thread::sleep(Duration::from_millis(1));
        }
        ratings.flush(Duration::from_secs(30));

        let written = photos.iter().filter(|p| sidecar::path_for(p).exists()).count();
        assert_eq!(written, photos.len(), "quitting would have cut the sync short");
        assert_eq!(syncing.join().unwrap().written, photos.len());
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn strings(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|t| t.to_string()).collect()
    }

    /// Typed as Lightroom has them typed: commas between tags, `>` or `|` between levels (record 0011).
    #[test]
    fn tags_read_as_typed() {
        assert_eq!(tags_from(" Travel >Portugal , Lisbon|Alfama,, christmas 2025 , Christmas 2025"), strings(&["christmas 2025", "Lisbon > Alfama", "Travel > Portugal"]));
        assert_eq!(tag_from(" > | "), None);
    }

    #[test]
    fn tags_are_added_removed_and_renamed_with_their_levels_below() {
        let data = temp_folder("tags");
        let (a, b) = (data.join("a.JPG"), data.join("b.JPG"));
        let ratings = Ratings::open(data.join("ratings.json"));
        let add = TagChange::Add { tags: strings(&["Travel > Portugal > Lisbon", "Family"]) };
        let changed = ratings.retag(&[a.clone(), b.clone()], &add).unwrap();
        assert_eq!(changed.before, vec![Vec::<String>::new(), Vec::new()]);
        assert_eq!(ratings.tags(&a), strings(&["Family", "Travel > Portugal > Lisbon"]));
        ratings.retag(&[b.clone()], &TagChange::Add { tags: strings(&["travel"]) }).unwrap();

        ratings.retag(&[a.clone(), b.clone()], &TagChange::Rename { from: "travel".into(), to: "Trips".into() }).unwrap();
        assert_eq!(ratings.tags(&a), strings(&["Family", "Trips > Portugal > Lisbon"]), "the levels below renamed with it");
        assert_eq!(ratings.tags(&b), strings(&["Family", "Trips", "Trips > Portugal > Lisbon"]));

        let removed = ratings.retag(&[a.clone(), b.clone()], &TagChange::Remove { tags: strings(&["trips"]) }).unwrap();
        assert_eq!((ratings.tags(&a), ratings.tags(&b)), (strings(&["Family"]), strings(&["Family"])), "and removed with it");
        ratings.set_tags(&[a.clone(), b.clone()], &removed.before).unwrap();
        assert_eq!(ratings.tags(&b), strings(&["Family", "Trips", "Trips > Portugal > Lisbon"]), "undone");

        // Kept in the file, read again.
        let again = Ratings::open(data.join("ratings.json"));
        assert_eq!(again.tags(&a), strings(&["Family", "Trips > Portugal > Lisbon"]));
        assert_eq!(again.get(&a), Rating::default(), "a photo with tags and no rating is kept");
        std::fs::remove_dir_all(data).unwrap();
    }

    /// A store from before tags reads as it was, and one without tags is written without them.
    #[test]
    fn a_store_from_before_reads_and_keeps_no_tags_key() {
        let data = temp_folder("tags-before");
        let photo = data.join("a.JPG");
        std::fs::write(data.join("ratings.json"), format!(r#"{{"{}": {{"pick": "picked", "stars": 2, "label": "none", "changedMs": 1}}}}"#, photo.display())).unwrap();
        let ratings = Ratings::open(data.join("ratings.json"));
        assert_eq!((ratings.get(&photo).stars, ratings.tags(&photo)), (2, Vec::new()));
        ratings.change(&[photo.clone()], picked(3)).unwrap();
        ratings.flush(Duration::from_secs(5));
        assert!(!std::fs::read_to_string(data.join("ratings.json")).unwrap().contains("\"tags\""), "no tags key");
        std::fs::remove_dir_all(data).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn tags_that_cant_be_saved_change_nothing() {
        let data = temp_folder("tags-unsaved");
        let photo = data.join("a.JPG");
        let ratings = Ratings::open(data.join("ratings.json"));
        ratings.retag(&[photo.clone()], &TagChange::Add { tags: strings(&["Kept"]) }).unwrap();
        set_writable(&data, false);
        let result = ratings.retag(&[photo.clone()], &TagChange::Add { tags: strings(&["Lost"]) });
        set_writable(&data, true);
        assert!(result.is_err());
        assert_eq!(ratings.tags(&photo), strings(&["Kept"]));
        std::fs::remove_dir_all(data).unwrap();
    }

    /// A photo's tags stay when another app changes its sidecar, through a sync and through a key
    /// pressed on top of that change: the sidecar doesn't hold them yet (record 0011).
    #[test]
    fn tags_stay_when_another_app_changes_the_sidecar() {
        let (dir, ratings, [raw, ..]) = library("tags-theirs");
        ratings.change(&[raw.clone()], picked(4)).unwrap();
        ratings.retag(&[raw.clone()], &TagChange::Add { tags: strings(&["Wedding"]) }).unwrap();
        ratings.flush(Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(20));
        sidecar::write(&dir.join("DSC1.xmp"), Rating { pick: Pick::Rejected, stars: 2, label: Label::Red }).unwrap();
        assert_eq!(ratings.sync(&[raw.clone()]).read, 1);
        assert_eq!(ratings.tags(&raw), strings(&["Wedding"]), "through a sync");

        std::thread::sleep(Duration::from_millis(20));
        sidecar::write(&dir.join("DSC1.xmp"), Rating { pick: Pick::Picked, stars: 1, label: Label::None }).unwrap();
        ratings.change(&[raw.clone()], picked(5)).unwrap();
        ratings.flush(Duration::from_secs(5));
        assert_eq!(ratings.tags(&raw), strings(&["Wedding"]), "through a key on top of another app's change");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
