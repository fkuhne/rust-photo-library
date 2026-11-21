//! Lists a folder as the library does (record 0007): how many photos, how many RAW files, how
//! many RAW+JPEG pairs, and what couldn't be opened.
//! Usage: cargo run --example list_folder -- <folder> [<folder>…]

fn main() {
    for folder in std::env::args().skip(1) {
        match photo_library::folder::list(std::path::Path::new(&folder)) {
            Ok(listing) => println!(
                "{folder}: {} photos, {} RAW, {} with a companion JPEG; {} couldn't be opened",
                listing.photos.len(),
                listing.photos.iter().filter(|p| p.raw).count(),
                listing.photos.iter().filter(|p| p.companion.is_some()).count(),
                listing.unlisted.len()
            ),
            Err(problem) => println!("{folder}: {problem}"),
        }
    }
}
