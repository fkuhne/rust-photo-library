# rust-photo-library

A Rust library, the crate `photo-library`, for desktop apps that work on a photographer's library of RAW files, on macOS and Windows. It's the shared core of three apps built with Tauri 2: PhotoMatch, for culling and finding photos, PhotoDev, for developing them, and PhotoBook, for laying them out for print. PhotoMatch uses it today; PhotoDev and PhotoBook move to it next.

It holds what those apps had been copying from one another, in one place, with its tests:

- **Reading photos:** RAW files from some 1,300 cameras through [LibRaw](https://www.libraw.org) 0.22.3, built from source; JPEG, PNG, WebP, TIFF, BMP and GIF through the `image` crate, with their EXIF; HEIC and AVIF through the system's own decoders, ImageIO on macOS and WIC on Windows. Behind one interface, each photo upright as its camera held it.
- **Thumbnails,** from a RAW file's embedded previews or the image itself, cached on disk, and RAW data developed at full size where a camera's preview is too small.
- **Ratings in XMP sidecars:** pick, stars, colour label and keywords, written beside each RAW file as Lightroom Classic writes them, everything else in a sidecar kept, and another app's changes taken in.
- **Files handled safely:** moving, copying, renaming and the Trash, each photo with its JPEG and its sidecar, never written over another file, and undone. The originals are never changed.
- **Checking photos from their RAW data,** not their previews:
  - **clipping:** the highlights that clip and the shadows that sink, from the sensor's own values;
  - **focus peaking:** where a photo is in focus and still, judged against the sensor's own noise in each band of brightness, so that sharpening isn't taken for focus, nor a blur of motion's streaks;
  - **the camera's focus point,** decoded from Sony's, Fujifilm's, Olympus's, Canon's and Nikon's maker notes, Nikon's named points placed by the Focus Points plugin's maps, and whether the photo is in focus there.
- **What tells a burst:** each photo's time to the millisecond, its camera's drive mode, by maker, and its body's serial number, the milliseconds kept by a small C++ callback on LibRaw's own reading of the file.
- **Where a photo was taken,** named offline: its town, region and country, from GeoNames' places, with the feature `places`.
- **Folder listing,** with a RAW file and the JPEG shot beside it shown as one photo.

About 7,200 lines of Rust, with 97 tests, 101 with `places`.

## Building and testing

It needs Rust (edition 2024) and a C++ compiler: Xcode's Command Line Tools on macOS, or Visual Studio's Build Tools on Windows. LibRaw is a git submodule, built by `cargo` itself; the first build compiles it, in about a minute.

```sh
git clone --recursive https://github.com/fkuhne/rust-photo-library.git
cd rust-photo-library
cargo test --lib                      # the tests
cargo test --lib --features places    # with the place names
```

An app takes it by git at a pinned commit:

```toml
[dependencies]
photo-library = { git = "https://github.com/fkuhne/rust-photo-library.git", rev = "<commit>", features = ["places"] }
```

The examples run it over a folder of photos, as an app would:

```sh
cargo run --release --example list_folder -- <folder>                    # the photos it lists
cargo run --release --example shot_details -- <folder>                   # time, drive mode, serial
cargo run --release --example focus_points -- <out> <folder>             # each camera's focus point, drawn
cargo run --release --example focus -- <out> <folder>                    # focus peaking, drawn
cargo run --release --example clipping -- <out> <folder>                 # clipping, drawn
cargo run --release --features places --example place_of -- <folder>    # where each was taken
```

## How it's worked on

From a spec, one plan step at a time, with a record for each decision:
- [`spec/product.md`](spec/product.md): what it covers and what it doesn't;
- [`spec/plan.md`](spec/plan.md): the steps, done and to come;
- [`spec/decisions/`](spec/decisions/README.md): why each choice was made, with its evidence;
- [`docs/inventory.md`](docs/inventory.md): what moved in from the apps, and every dependency's licence.

[`AGENTS.md`](AGENTS.md) gives the method to coding agents, which work on it with the owner.

## Credits

It builds in others' work, each under its own licence:
- **[LibRaw](https://github.com/LibRaw/LibRaw)** 0.22.3, a submodule of LibRaw's own repository, dual-licensed CDDL 1.0 or LGPL 2.1, used here under CDDL 1.0.
- **`rsraw` and `rsraw-sys`** 0.1.1, by hexilee ([hexilee/rsraw](https://github.com/hexilee/rsraw)), MIT, vendored with additions of ours, each marked; their licence in `vendor/`.
- **The Focus Points plugin's maps** of Nikon's AF points ([musselwhizzle/Focus-Points](https://github.com/musselwhizzle/Focus-Points)), Apache 2.0, built in unchanged, their licence in `src/af_maps/`.
- **Place names from [GeoNames](https://www.geonames.org),** under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), cut to the columns a lookup needs, in `data/places.tsv.gz`.
- **ExifTool's tag documentation,** by Phil Harvey ([exiftool.org](https://exiftool.org/TagNames/)), from which the maker notes' layouts and values were read; none of its code is used.

Every crate it depends on is under a permissive licence, as each one's manifest says: MIT, Apache 2.0 or both for most, and BSD, ISC, Zlib or the Unlicense for the rest. `xmp_toolkit` builds in Adobe's XMP Toolkit, under BSD 3-Clause, and libexpat, under MIT.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option. The works credited above keep their own licences.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
