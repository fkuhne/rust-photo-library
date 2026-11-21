//! The names PhotoDev and PhotoMatch use from the library, reached from outside it as an app
//! does, so that a change that hides one fails here first (the library's plan step 1.2). PhotoDev's
//! list is in its handoff of 2026-11-20; PhotoMatch's is checked when it moves, in step 2.1.

use photo_library::problem::Problem;
use photo_library::{files, folder, photo, raw, rsraw, thumbs};

#[test]
fn the_names_photodev_uses_are_public() {
    // photo: the readers, what to read, the details, which files, RAW+JPEG pairs.
    let _readers: &dyn photo::PhotoReader = &photo::Readers;
    let _ = (photo::Want::Full, photo::PhotoInfo::default());
    let _ = (photo::is_supported, photo::pair);
    // thumbs: the cache, its size, and the image helpers PhotoDev turns, decodes and encodes with.
    let _ = (thumbs::ThumbCache::new, thumbs::ThumbCache::thumbnail, thumbs::ThumbCache::info);
    let _ = (thumbs::THUMB_SIZE, thumbs::upright, thumbs::decode, thumbs::encode);
    // files, raw, and the folder listing that replaces PhotoDev's `folder.rs` (record 0007).
    let _ = (files::write_atomically, files::write_durably, raw::is_raw);
    let _: fn(&std::path::Path) -> Result<folder::Listing, Problem> = folder::list;
    let _ = |entry: folder::Entry| (entry.path, entry.companion, entry.raw);
    // rsraw: PhotoDev's own additions, from the library's copy.
    let _ = (rsraw::RawImage::set_output_color, rsraw::RawImage::set_demosaic, rsraw::RawImage::dcraw_process);
    let _ = (rsraw::RawImage::decoder, rsraw::RawImage::libraw_version, rsraw::RawImage::write_tiff);
}

#[test]
fn a_problem_says_itself_and_its_kinds_are_named_from_outside() {
    let problem = Problem::Damaged { cause: "cut short".into() };
    assert!(!problem.to_string().is_empty());
    // Outside the library, a match needs a wildcard arm: a kind added later breaks nothing.
    let named = match problem {
        Problem::PhotoGone { .. } | Problem::Damaged { .. } | Problem::NotAPhoto { .. } => true,
        _ => false,
    };
    assert!(named);
}
