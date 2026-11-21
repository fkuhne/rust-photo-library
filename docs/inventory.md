# The inventory: what the apps copied, and what moves in

What each of the owner's photo apps had copied from the others, where it came from, how the copies differ, and the licences of what moves into the library. Each claim names its source and date: a file at a commit, a command run, or a vendor page. A number worked out but not run is marked "computed"; a claim from memory, "unchecked".

First pass on 2026-11-20, before anything moved.

## PhotoMatch's library code

At PhotoMatch's commit `b959990`, 2026-11-20, in `app/src-tauri/src/`, from `wc -l`:

| File | Lines | What it does |
|---|---|---|
| `files.rs` | 39 | Writing files atomically and durably, through a temporary file |
| `photo.rs` | 187 | The photo interface: previews, orientation, details, which reader, RAW+JPEG pairs |
| `raw.rs` | 247 | RAW files through LibRaw: previews taken out, details, developing when there's no preview |
| `image_file.rs` | 192 | Image files through the `image` crate, with EXIF |
| `os_image.rs` | 330 | HEIC, HEIF and AVIF through the OS: ImageIO on macOS, WIC on Windows |
| `thumbs.rs` | 495 | Thumbnails and details cached on disk; the large view's images |
| `problem.rs` | 178 | Problems as kinds, for the interface to word |
| `ratings.rs` | 769 | Pick, stars and label, kept in PhotoMatch's store |
| `sidecar.rs` | 260 | XMP sidecars beside RAW files, by PhotoMatch's record 0014 |
| **Total** | **2,697** | |

Not everything in `ratings.rs` belongs in the library: PhotoMatch's own store of ratings, for instance, may stay PhotoMatch's. Step 1.4 decides.

Some of it is recent, and PhotoMatch's own so far (its notes, read on 2026-11-20). It moves as it is, and step 1.1 decides what the library shows of it:
- **In `thumbs.rs`:** the limit on the full-size photos kept, 2 GB, oldest first, and making one ahead of a zoom (`prepare_full`, `trim_full`, the counters `fulls_made` and `prepared`; PhotoMatch's records 0034 and 0035).
- **In `problem.rs`:** `NeedsWindowsExtension`'s `codec_only`, for a HEIC whose HEVC codec Windows lacks (PhotoMatch's record 0028).

### PhotoMatch's records behind the code

The code moves as it is ([0004](../spec/decisions/0004-photomatch-s-code-as-it-is-is-the-starting-point.md)), and these records of PhotoMatch's say why it's as it is:

| Record | Title | Behind |
|---|---|---|
| 0007 | Read RAW files with LibRaw, behind our own interface | `photo.rs`, `raw.rs` |
| 0010 | Read image files too, behind the same interface | `image_file.rs` |
| 0012 | HEIC and AVIF through the OS's own decoders | `os_image.rs` |
| 0013 | Build LibRaw 0.22.3, from a pinned copy | `rsraw-sys`, the submodule |
| 0014 | Hand off ratings through XMP sidecars beside RAW files | `sidecar.rs`, `ratings.rs` |
| 0015 | Viewing quality before speed | `thumbs.rs`, developing at full size |
| 0016 | Read only the part of a RAW file a view needs | `raw.rs`, `rsraw`'s `open_file` |
| 0022 | Errors as kinds from the core, worded in one file of the interface | `problem.rs` |
| 0028 | Windows checked on the owner's PC; CI on tags and by hand | `os_image.rs`'s WIC, `problem.rs` |
| 0031 | A library measured by a switch in the app; no content hash until the photo database | `rsraw`'s `open_file` on Windows |
| 0033 | Full-size development keeps AHD; the wait is for developing ahead to shorten | `raw.rs` |
| 0034 | The large view develops a photo's full size ahead of a zoom, once it has stayed on it | `thumbs.rs` |
| 0035 | The large view zooms by small steps, with the wheel, a pinch and keys | `thumbs.rs`'s counter `prepared` |

## PhotoDev's copy

PhotoDev copied the first seven files at PhotoMatch's commit `9217996`, into `app/core/src/library/` (PhotoDev's commit `a539dc7`), 1,668 lines.

Compared with PhotoMatch's files at `b959990` on 2026-11-20, after the same path changes, they differ only by PhotoDev's own small changes:
- `photo.rs`, `raw.rs`, `image_file.rs`, `files.rs`: no line differs.
- `os_image.rs`: 4 lines, two paths in nested modules.
- `thumbs.rs`: 3 lines, `upright` made visible, with a comment.
- `problem.rs`: 6 lines, paths in its test module.

So PhotoMatch's seven files didn't change between `9217996` and `b959990`.

PhotoDev's own library code, written for it, may move in too:
- `folder.rs`, folder listing;
- `edits.rs`, edits files beside photos.

The first belongs in the library. The second is PhotoDev's.

## The two `rsraw` copies

Both are copies of `rsraw` 0.1.1 and `rsraw-sys` 0.1.1 (MIT, `hexilee/rsraw`), with LibRaw 0.22.3 as a git submodule, pinned to commit `cf02d66`.

- **PhotoMatch's additions,** from the comments marking them in `vendor/rsraw/src/`:
  - `open_file`, through LibRaw's own reader, closing the handle when opening fails;
  - `list_thumbs`, `extract_thumb` and `ThumbListing`;
  - the error type, exported;
  - `open_file` on Windows, through LibRaw's wide-character call `libraw_open_wfile`, so that a path with any letter opens (its record 0031, plan step 2.2). Its binding exists only when bindgen runs for Windows: the checked-in `src/bindings.rs`, made on a Mac, lacks it.

  Its plan steps 2.24 and 2.2.
- **PhotoDev's additions,** in its `vendor/rsraw/src/raw.rs`, each marked:
  - LibRaw's processing options: `set_output_color`, `set_output_bps`, `set_gamma`, `set_no_auto_bright`, `set_highlight` and `set_demosaic`;
  - `dcraw_process`, `output_size` and `write_tiff`;
  - `color_data`, `dng_baseline_exposure`, `decoder` and `libraw_version`.

  Its plan steps 1.2 to 2.1.
- **PhotoDev's copy holds PhotoMatch's additions too,** as it was copied from PhotoMatch's, but from its commit `c12880a`, older than the rest of PhotoDev's copy: it lacks PhotoMatch's `open_file` on Windows (its record 0031). So the merged `rsraw` is PhotoMatch's, at its commit `391fd74`, with PhotoDev's block of eleven functions taken in from its commit `d95ff89`, each addition marked by whose it was (step 1.2). Otherwise the two differed only by comments.
- **`rsraw-sys`:** the two build scripts and manifests differ by 6 lines, PhotoDev's header comment.

## PhotoBook's photo module

PhotoBook's `app/src-tauri/src/photo/`: `mod.rs` and `tests.rs`, 338 lines, from `wc -l` on 2026-11-20. It reads JPEG and PNG files with their EXIF orientation, derived from PhotoMatch's. It moves in next, after PhotoMatch and PhotoDev ([`product.md`](../spec/product.md#scope)).

## Licences of what moves in

From PhotoMatch's `docs/licences.md`, read on 2026-11-20, and PhotoDev's checks of the same versions:

| Dependency | Version | Licence | Allows MIT or Apache 2.0 publishing, and closed use |
|---|---|---|---|
| `rsraw`, `rsraw-sys` | 0.1.1, our copies | MIT | Yes |
| LibRaw | 0.22.3 | CDDL 1.0 or LGPL 2.1, used under CDDL 1.0 | Yes: CDDL is file by file. Its source must be offered with binaries, as PhotoMatch does (its `docs/licences.md`) |
| `image` | 0.25.10 | MIT or Apache 2.0 | Yes |
| `jpeg-decoder` | 0.3.2 | MIT or Apache 2.0 | Yes |
| `kamadak-exif` | 0.6.1 | BSD 2-Clause | Yes |
| `blake3` | 1.8.7 | CC0 1.0, Apache 2.0, or Apache 2.0 with LLVM's exception | Yes |
| `xmp_toolkit` | 1.12.1 | MIT or Apache 2.0; it builds Adobe's XMP Toolkit, BSD 3-Clause, with expat and zlib (PhotoMatch's `docs/licences.md`) | Yes (unchecked for the toolkit's own terms beyond its notices) |
| `serde`, `serde_json` | 1.0.229, 1.0.154 | MIT or Apache 2.0 | Yes |
| `windows`, on Windows | 0.62.2 | MIT or Apache 2.0 (unchecked here) | Yes |

None is GPL or AGPL.

### The skeleton's whole graph

What plan step 1.1 builds: `photo-library`, the vendored `rsraw` and `rsraw-sys`, and 63 packages from crates.io, with the versions PhotoMatch's `Cargo.lock` has at commit `391fd74`, from which the lockfile was seeded. Their licences, from `cargo metadata` on 2026-11-21:
- **MIT or Apache 2.0, one or both:** 55 packages, among them `cc` 1.10.0, `libc` 0.2.191, `chrono` 0.4.45, `serde` 1.0.229 and `regex` 1.13.1. `tracing` 0.1.44, `nom` 7.1.3 and `slab` 0.4.12 are MIT alone; `unicode-ident` adds Unicode 3.0.
- **BSD 3-Clause:** `bindgen` 0.71.1, a build tool that writes `rsraw-sys`'s bindings.
- **Apache 2.0:** `clang-sys` 1.9.1, which `bindgen` uses.
- **ISC:** `libloading` 0.8.9.
- **Unlicense or MIT:** `aho-corasick` 1.1.5 and `memchr` 2.8.3.
- **MIT, Apache 2.0, or LGPL 2.1 or later:** `r-efi` 6.0.0, a dependency of `getrandom` for UEFI targets, used under MIT or Apache 2.0.

Every one allows MIT or Apache 2.0 publishing and closed use.

Step 1.4 adds `xmp_toolkit` 1.12.1, MIT or Apache 2.0, which builds Adobe's XMP Toolkit, BSD 3-Clause, with expat and zlib (PhotoMatch's `docs/licences.md`), and 14 more packages for it, among them `thiserror`, `num_enum`, `fs_extra` and `toml_edit`: each MIT, Apache 2.0 or BSD 3-Clause, from `cargo metadata` on 2026-11-21; 123 packages from crates.io in all, every one at PhotoMatch's version.

Step 1.3 adds `walkdir` 2.5.0 and `same-file` 1.0.6, both Unlicense or MIT, and on Windows `winapi-util` 0.1.11, Unlicense or MIT, and `windows-sys` 0.61.2, MIT or Apache 2.0: 109 packages from crates.io in all, from `cargo metadata` on 2026-11-21, at PhotoMatch's versions.

## What moved, and what changed on the way

Plan step 1.2, on 2026-11-21, from PhotoMatch's commit `391fd74`: `photo.rs`, `raw.rs`, `image_file.rs`, `os_image.rs`, `thumbs.rs`, `problem.rs`, and `files.rs` with them, as `thumbs.rs` writes its files through it. Each moved as it was, with its tests, but for these, each marked in the code:
- **`Problem` is `#[non_exhaustive]`** ([0005](../spec/decisions/0005-a-workspace-of-three-packages-libraw-always-built-photomatch-s-modules.md)): an app's `match` on it needs a wildcard arm.
- **`thumbs::upright`, `thumbs::decode` and `thumbs::encode` are public:** PhotoDev uses them, and could only because its copy was inside its own crate. In PhotoMatch they were the crate's own.
- **`rsraw` and `rsraw_sys` are re-exported** at the crate's root, so an app reaches the library's own copy and needs no dependency of its own on them. Rust's API guidelines say a dependency whose types a crate's public API uses is part of that API (C-STABLE); they say nothing of re-exporting, so this is the library's choice.
- **The dependencies** came in at PhotoMatch's versions ([0006](../spec/decisions/0006-dependencies-as-default-requirements-the-lockfile-for-the-library-s-own-builds.md)).

The thumbnail cache's format, `CACHE_VERSION` included, and `Problem`'s kinds and their JSON are unchanged.

## For the apps

What an app needs, as of step 1.4:
- **The modules are PhotoMatch's,** under the crate's name: `photo_library::photo`, `::raw`, `::image_file`, `::os_image`, `::thumbs`, `::problem`, `::files`, and `photo_library::rsraw` for LibRaw. And `photo_library::folder`, both apps' listing made one ([0007](../spec/decisions/0007-one-folder-listing-from-both-apps-hidden-files-left-out.md)): `list(folder)` gives a `Listing`, the photos as `Entry` (path, companion, `raw`) and the paths that couldn't be opened.
- **`photo_library::ratings` and `::sidecar`,** PhotoMatch's as they were ([0008](../spec/decisions/0008-ratings-the-model-the-store-and-the-sidecars-move-whole.md)): an app opens its own store, `Ratings::open(file)`, at a file in its data folder, and rates through it; a RAW file's rating reaches its sidecar, and comes back from it when the other app changed it. An image file's rating stays in the app that gave it.
- **PhotoDev,** in place of its `folder.rs`: it sets `edited` itself from its edits files, and takes the library's kinds for a folder's problems, `FolderMissing`, `FolderNotAllowed` and `FolderUnreadable`, in place of its `NotFound` and `Unreadable`.
- **PhotoMatch** is on the library since 2026-11-21, at `9e1b65d` (its commit `251ea86`, its record 0038), the library's `folder::list` in place of its own `list`: the same walk and report, hidden files and folders left out. Every name in PhotoDev's list of 2026-11-20 is public, which [`tests/public_api.rs`](../tests/public_api.rs) checks.
- **To build:** the library by git at a commit, nothing else of LibRaw's: Cargo fetches its submodule with it (the Cargo book). And on the machine, a C and C++ compiler, and libclang for `bindgen`: on a Mac, Xcode's Command Line Tools.
- **A `match` on `Problem`** gets a wildcard arm.

## Open questions

- Does PhotoMatch answer its thumbnail requests in a way that can abort, as PhotoDev's did? Step 2.1, in PhotoMatch's code.
- How PhotoMatch's CI fetches the private library ([0002](../spec/decisions/0002-one-rust-crate-in-its-own-repository-pinned-by-each-app.md)). PhotoMatch's installers are built by CI, so its next release waits on it.
