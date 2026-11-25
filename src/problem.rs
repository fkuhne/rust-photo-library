//! What went wrong, by kind, for the interface to say in a photographer's words (plan step 2.35,
//! record 0022). The words are the interface's, in `app/src/messages.ts`, listed in
//! `docs/messages.md`; the core keeps the technical cause, for Help → Diagnostics and reports.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// A problem the interface can be told of, tagged by its kind in JSON: `{"kind": "Damaged",
/// "cause": "…"}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
// photo-library: non-exhaustive, so that a kind added later breaks no app's `match` (record 0005).
#[non_exhaustive]
pub enum Problem {
    /// The folder asked for isn't there: a card or a drive taken out, or the folder moved.
    FolderMissing { folder: String },
    /// The system refuses to list the folder.
    FolderNotAllowed { folder: String },
    /// Any other failure to list the folder.
    FolderUnreadable { folder: String, cause: String },
    /// The photo's file isn't where it was when the folder opened.
    PhotoGone { cause: String },
    /// The system refuses to open the photo's file.
    PhotoNotAllowed { cause: String },
    /// The file's image can't be decoded: cut off or corrupted.
    Damaged { cause: String },
    /// LibRaw doesn't recognise a file named as a RAW file, or the image crate an image file.
    NotAPhoto { cause: String },
    /// A RAW file with no preview PhotoMatch can show, from a camera it can't develop.
    NoPreviewCamera { camera: String },
    /// A RAW file with no preview PhotoMatch can show, whose developing failed.
    DevelopFailed { cause: String },
    /// A HEIC or AVIF file Windows can't decode, as one of Microsoft's extensions is missing:
    /// `format` says which, "HEIC" or "AVIF" (record 0028). `codec_only` when Windows read the
    /// file but has no working codec for its image: for a HEIC, the HEVC extension alone is needed,
    /// not the HEIF one (step 2.1, on the owner's PC).
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    NeedsWindowsExtension {
        format: String,
        #[serde(rename = "codecOnly")]
        codec_only: bool,
        cause: String,
    },
    /// The disk refuses an XMP sidecar in a folder: a locked card, a read-only folder, a full disk.
    SidecarsNotWritable { folder: String, cause: String },
    /// An XMP sidecar beside a photo that can't be read, and is left as it is.
    SidecarUnreadable { photo: String, cause: String },
    /// The ratings file can't be written.
    RatingsNotSaved { cause: String },
    /// At launch, the ratings file can't be understood, and is kept aside as `kept_as`.
    RatingsFileDamaged {
        #[serde(rename = "keptAs")]
        kept_as: String,
        cause: String,
    },
    /// At launch, the ratings file can't be opened, nor kept aside: the ratings are kept in
    /// memory only, and that file is never written over.
    RatingsFileUnreadable { file: String, cause: String },
    /// The settings file can't be written.
    SettingsNotSaved { cause: String },
    /// A photo's files can't be moved to the Trash, and stay where they are (record 0015).
    NotTrashed { photo: String, cause: String },
    /// A photo's files can't be moved back from the Trash, and stay there (record 0015).
    NotPutBack { photo: String, cause: String },
    /// A photo's files can't be moved to another folder, and stay where they are (record 0016).
    NotMoved { photo: String, cause: String },
    /// A photo's files can't be copied to another folder; any copy made is removed (record 0016).
    NotCopied { photo: String, cause: String },
    /// Undo can't move a photo back where it was, nor its copy away (record 0016).
    NotMovedBack { photo: String, cause: String },
    /// A photo's files can't be renamed, and keep their names (record 0017).
    NotRenamed { photo: String, cause: String },
    /// An answer for a folder that has since closed: nothing to show.
    Stale,
    /// Anything that shouldn't happen.
    Internal { cause: String },
}

impl Problem {
    /// A photo's file that can't be read: gone, not allowed, or something else.
    pub fn reading(path: &Path, error: std::io::Error) -> Self {
        let cause = format!("can't read {}: {error}", path.display());
        match error.kind() {
            std::io::ErrorKind::NotFound => Problem::PhotoGone { cause },
            std::io::ErrorKind::PermissionDenied => Problem::PhotoNotAllowed { cause },
            _ => Problem::Internal { cause },
        }
    }

    /// The technical cause, for Diagnostics and the app's report.
    pub fn cause(&self) -> String {
        match self {
            Problem::FolderMissing { folder } => format!("{folder} isn't there"),
            Problem::FolderNotAllowed { folder } => format!("not allowed to list {folder}"),
            Problem::RatingsFileDamaged { kept_as, cause } => format!("the ratings file can't be understood ({cause}); kept as {kept_as}"),
            Problem::RatingsFileUnreadable { file, cause } => format!("{file}: {cause}"),
            Problem::FolderUnreadable { cause, .. }
            | Problem::SidecarsNotWritable { cause, .. }
            | Problem::SidecarUnreadable { cause, .. }
            | Problem::RatingsNotSaved { cause }
            | Problem::SettingsNotSaved { cause }
            | Problem::NotTrashed { cause, .. }
            | Problem::NotPutBack { cause, .. }
            | Problem::NotMoved { cause, .. }
            | Problem::NotCopied { cause, .. }
            | Problem::NotMovedBack { cause, .. }
            | Problem::NotRenamed { cause, .. } => cause.clone(),
            Problem::PhotoGone { cause }
            | Problem::PhotoNotAllowed { cause }
            | Problem::Damaged { cause }
            | Problem::NotAPhoto { cause }
            | Problem::DevelopFailed { cause }
            | Problem::NeedsWindowsExtension { cause, .. }
            | Problem::Internal { cause } => cause.clone(),
            Problem::NoPreviewCamera { camera } => format!("no preview PhotoMatch can show, and LibRaw has no colours for the {camera}"),
            Problem::Stale => "an answer for a folder that has since closed".into(),
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.cause())
    }
}

impl std::error::Error for Problem {}

/// An error not sorted into a kind yet, such as a lock's or a thread's.
impl From<String> for Problem {
    fn from(cause: String) -> Self {
        Problem::Internal { cause }
    }
}

impl From<&str> for Problem {
    fn from(cause: &str) -> Self {
        Problem::Internal { cause: cause.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_problem_reaches_the_interface_tagged_by_its_kind() {
        let damaged = Problem::Damaged { cause: "cut off".into() };
        assert_eq!(serde_json::to_string(&damaged).unwrap(), r#"{"kind":"Damaged","cause":"cut off"}"#);
        assert_eq!(serde_json::to_string(&Problem::Stale).unwrap(), r#"{"kind":"Stale"}"#);
        let back: Problem = serde_json::from_str(r#"{"kind":"NoPreviewCamera","camera":"Canon EOS R5 Mark II"}"#).unwrap();
        assert_eq!(back, Problem::NoPreviewCamera { camera: "Canon EOS R5 Mark II".into() });
    }

    #[test]
    fn a_file_that_cant_be_read_is_gone_or_not_allowed() {
        let path = Path::new("/nowhere/DSC1.ARW");
        let gone = Problem::reading(path, std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(matches!(gone, Problem::PhotoGone { .. }), "{gone:?}");
        let refused = Problem::reading(path, std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert!(matches!(refused, Problem::PhotoNotAllowed { .. }), "{refused:?}");
        assert!(gone.cause().contains("DSC1.ARW"), "the cause names the file, for Diagnostics");
    }

    /// What the readers make of the broken-files samples' kinds of file, made here: a JPEG cut
    /// short, a file of noise named as a RAW file, and a file that isn't there. A JPEG cut short
    /// may fail as its header is read or as its image is decoded; either way it's damaged.
    #[test]
    fn the_readers_say_what_kind_of_problem_a_file_has() {
        use crate::photo::{PhotoReader, Readers, Want};
        let dir = std::env::temp_dir().join(format!("photomatch-problems-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let jpeg = crate::thumbs::encode(&image::DynamicImage::new_rgb8(64, 48), 80).unwrap();
        let cut = dir.join("Cut.jpg");
        std::fs::write(&cut, &jpeg[..jpeg.len() / 3]).unwrap();
        let problem = match Readers.read(&cut, Want::AtLeast(800)) {
            Err(problem) => problem,
            Ok(photo) => crate::thumbs::make_thumbnail(&photo).err().expect("a JPEG cut short can't make a thumbnail"),
        };
        assert!(matches!(problem, Problem::Damaged { .. }), "{problem:?}");

        let noise = dir.join("Noise.ARW");
        let bytes: Vec<u8> = (0..65_536u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        std::fs::write(&noise, bytes).unwrap();
        let problem = Readers.read(&noise, Want::AtLeast(800)).err().expect("noise isn't a RAW file");
        assert!(matches!(problem, Problem::NotAPhoto { .. }), "{problem:?}");

        for name in ["Gone.ARW", "Gone.jpg"] {
            let problem = Readers.read(&dir.join(name), Want::AtLeast(800)).err().expect("a file that isn't there");
            assert!(matches!(problem, Problem::PhotoGone { .. }), "{name}: {problem:?}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
