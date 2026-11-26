//! Each photo's focus marks, as an app shows them over its large view (the library's plan step
//! 2.13, record 0018): from a RAW file's sensor data, from an image file's pixels. Times each at
//! every level, says how much of the photo is marked, and writes the photo with its Mid marks
//! over it into `<out>`, to look at.
//!
//! cargo run --release --example focus -- <out> <folder or file>…

use std::path::{Path, PathBuf};
use std::time::Instant;

use image::ImageFormat;
use photo_library::focus::{self, Level};
use photo_library::photo::Readers;
use photo_library::thumbs::ThumbCache;
use photo_library::{folder, raw};

/// The large view's size on a screen 2560 pixels across.
const SCREEN: u32 = 2560;
const GREEN: [u8; 3] = [0, 255, 0];

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("a folder to write into, then folders or files"));
    std::fs::create_dir_all(&out).expect("the folder to write into");
    let mut paths = Vec::new();
    for arg in args {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            paths.extend(folder::list(&path).expect("a folder").photos.into_iter().map(|e| e.path));
        } else {
            paths.push(path);
        }
    }
    let cache = ThumbCache::new(out.join("cache")).expect("a cache");
    println!("photo | shown | Mid made in | marked at Low · Mid · High");
    for path in paths {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let shown = match cache.screen(&Readers, &path, SCREEN).ok().and_then(|(bytes, _)| image::load_from_memory(&bytes).ok()) {
            Some(image) => image,
            None => {
                println!("{name} | no large view");
                continue;
            }
        };
        let size = (shown.width(), shown.height());
        let mark = |level| if raw::is_raw(&path) { focus::of_raw(&path, size, level) } else { Ok(focus::of_image(&shown, size, level)) };
        let start = Instant::now();
        let mid = mark(Level::Mid);
        let ms = start.elapsed().as_millis();
        match mid {
            Ok(marks) => {
                let share = |marks: &focus::Marks| marks.count() as f64 * 100.0 / marks.sharp.len().max(1) as f64;
                let others: Vec<String> = [Level::Low, Level::High].into_iter().map(|level| mark(level).map(|m| format!("{:.2}%", share(&m))).unwrap_or_else(|_| "-".into())).collect();
                println!("{name} | {} × {} | {ms} ms | {} · {:.2}% · {}", marks.width, marks.height, others[0], share(&marks), others[1]);
                write_over(&out, &name, &shown, &marks);
            }
            Err(problem) => println!("{name} | {} × {} | {ms} ms | {problem:?}", size.0, size.1),
        }
    }
}

fn write_over(out: &Path, name: &str, shown: &image::DynamicImage, marks: &focus::Marks) {
    let mut photo = shown.to_rgba8();
    let over = image::load_from_memory_with_format(&marks.png(GREEN).expect("a PNG"), ImageFormat::Png).expect("the marks").to_rgba8();
    let over = image::imageops::resize(&over, photo.width(), photo.height(), image::imageops::FilterType::Nearest);
    image::imageops::overlay(&mut photo, &over, 0, 0);
    let _ = image::DynamicImage::ImageRgba8(photo).to_rgb8().save(out.join(format!("{name}.jpg")));
}
