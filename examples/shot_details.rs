//! What each photo records that tells a burst, as the library reads its details (record 0023): the
//! time it was taken with its milliseconds, the drive mode and the camera's serial number, and how
//! long reading the details took.
//!
//! cargo run --release --example shot_details -- <folder or file>…

use std::path::PathBuf;
use std::time::Instant;

use photo_library::folder;
use photo_library::photo::{PhotoReader, Readers, Want};

fn main() {
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            paths.extend(folder::list(&path).expect("a folder").photos.into_iter().map(|e| e.path));
        } else {
            paths.push(path);
        }
    }
    for path in paths {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let started = Instant::now();
        match Readers.read(&path, Want::Details) {
            Ok(photo) => {
                let info = photo.info;
                let ms = info.captured_ms.map_or("—".to_string(), |ms| format!(".{ms:03}"));
                let drive = info.drive.map_or("—".to_string(), |d| format!("{d:?}"));
                println!(
                    "{name}: {} {}; {}{ms}; drive {drive}; serial {:?}; {:.0} ms",
                    info.make,
                    info.model,
                    info.captured_at.as_deref().unwrap_or("no time"),
                    info.serial,
                    started.elapsed().as_secs_f64() * 1000.0
                );
            }
            Err(problem) => println!("{name}: {problem}"),
        }
    }
}
