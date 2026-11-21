//! Names where each photo in a folder was taken, as an app with the feature `places` does: its
//! position from the file, or else from its sidecar, and its town, region and country (record
//! 0010). Says how long the place list took to read.
//!
//! cargo run --release --features places --example place_of -- <folder>

use std::path::Path;
use std::time::Instant;

use photo_library::photo::{PhotoReader, Readers, Want};
use photo_library::{folder, places, sidecar};

fn main() {
    let root = std::env::args().nth(1).expect("a folder");
    let listing = folder::list(Path::new(&root)).expect("a folder to list");
    let mut first = true;
    for entry in listing.photos {
        let name = entry.path.strip_prefix(&root).unwrap_or(&entry.path).display().to_string();
        let from_file = Readers.read(&entry.path, Want::Details).ok().and_then(|p| p.info.position);
        let (position, source) = match from_file {
            Some(p) => (Some(p), "file"),
            None => (sidecar::position(&entry.path), "sidecar"),
        };
        let Some(position) = position else {
            println!("{name}: no position");
            continue;
        };
        let started = Instant::now();
        let place = places::place_of(position);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let said = match place {
            Some(p) => [p.town, p.region, Some(p.country)].into_iter().flatten().collect::<Vec<_>>().join(", "),
            None => "no place".into(),
        };
        println!("{name}: {:.4}, {:.4} ({source}) → {said}{}", position.latitude, position.longitude, if first { format!(" (the list read, and the lookup, in {ms:.0} ms)") } else { format!(" ({:.3} ms)", ms) });
        first = false;
    }
}
