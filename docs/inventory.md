# The inventory: what the apps copied, and what moves in

What each of the owner's photo apps had copied from the others, where it came from, how the copies differ, and the licences of what moves into the library. Each claim names its source and date: a file at a commit, a command run, or a vendor page. A number worked out but not run is marked "computed"; a claim from memory, "unchecked".

First pass on 2026-11-20, before anything moved.

## Photomatch's library code

At Photomatch's commit `b959990`, 2026-11-20, in `app/src-tauri/src/`, from `wc -l`:

| File | Lines | What it does |
|---|---|---|
| `files.rs` | 39 | Writing files atomically and durably, through a temporary file |
| `photo.rs` | 187 | The photo interface: previews, orientation, details, which reader, RAW+JPEG pairs |
| `raw.rs` | 247 | RAW files through LibRaw: previews taken out, details, developing when there's no preview |
| `image_file.rs` | 192 | Image files through the `image` crate, with EXIF |
| `os_image.rs` | 330 | HEIC, HEIF and AVIF through the OS: ImageIO on macOS, WIC on Windows |
| `thumbs.rs` | 495 | Thumbnails and details cached on disk; the large view's images |
| `problem.rs` | 178 | Problems as kinds, for the interface to word |
| `ratings.rs` | 769 | Pick, stars and label, kept in Photomatch's store |
| `sidecar.rs` | 260 | XMP sidecars beside RAW files, by Photomatch's record 0014 |
| **Total** | **2,697** | |

Not everything in `ratings.rs` belongs in the library: Photomatch's own store of ratings, for instance, may stay Photomatch's. Step 1.4 decides.

Some of it is recent, and Photomatch's own so far (its notes, read on 2026-11-20). It moves as it is, and step 1.1 decides what the library shows of it:
- **In `thumbs.rs`:** the limit on the full-size photos kept, 2 GB, oldest first, and making one ahead of a zoom (`prepare_full`, `trim_full`, the counters `fulls_made` and `prepared`; Photomatch's records 0034 and 0035).
- **In `problem.rs`:** `NeedsWindowsExtension`'s `codec_only`, for a HEIC whose HEVC codec Windows lacks (Photomatch's record 0028).

### Photomatch's records behind the code

The code moves as it is ([0004](../spec/decisions/0004-photomatch-s-code-as-it-is-is-the-starting-point.md)), and these records of Photomatch's say why it's as it is:

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

PhotoDev copied the first seven files at Photomatch's commit `9217996`, into `app/core/src/library/` (PhotoDev's commit `a539dc7`), 1,668 lines.

Compared with Photomatch's files at `b959990` on 2026-11-20, after the same path changes, they differ only by PhotoDev's own small changes:
- `photo.rs`, `raw.rs`, `image_file.rs`, `files.rs`: no line differs.
- `os_image.rs`: 4 lines, two paths in nested modules.
- `thumbs.rs`: 3 lines, `upright` made visible, with a comment.
- `problem.rs`: 6 lines, paths in its test module.

So Photomatch's seven files didn't change between `9217996` and `b959990`.

PhotoDev's own library code, written for it, may move in too:
- `folder.rs`, folder listing;
- `edits.rs`, edits files beside photos.

The first belongs in the library. The second is PhotoDev's.

## The two `rsraw` copies

Both are copies of `rsraw` 0.1.1 and `rsraw-sys` 0.1.1 (MIT, `hexilee/rsraw`), with LibRaw 0.22.3 as a git submodule, pinned to commit `cf02d66`.

- **Photomatch's additions,** from the comments marking them in `vendor/rsraw/src/`:
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
- **PhotoDev's copy holds Photomatch's additions too,** as it was copied from Photomatch's. So the merged `rsraw` is PhotoDev's, with each addition marked by whose it was.
- **`rsraw-sys`:** the two build scripts and manifests differ by 6 lines, PhotoDev's header comment.

## PhotoBook's photo module

PhotoBook's `app/src-tauri/src/photo/`: `mod.rs` and `tests.rs`, 338 lines, from `wc -l` on 2026-11-20. It reads JPEG and PNG files with their EXIF orientation, derived from Photomatch's. It moves in next, after Photomatch and PhotoDev ([`product.md`](../spec/product.md#scope)).

## Licences of what moves in

From Photomatch's `docs/licences.md`, read on 2026-11-20, and PhotoDev's checks of the same versions:

| Dependency | Version | Licence | Allows MIT or Apache 2.0 publishing, and closed use |
|---|---|---|---|
| `rsraw`, `rsraw-sys` | 0.1.1, our copies | MIT | Yes |
| LibRaw | 0.22.3 | CDDL 1.0 or LGPL 2.1, used under CDDL 1.0 | Yes: CDDL is file by file. Its source must be offered with binaries, as Photomatch does (its `docs/licences.md`) |
| `image` | 0.25.10 | MIT or Apache 2.0 | Yes |
| `jpeg-decoder` | 0.3.2 | MIT or Apache 2.0 | Yes |
| `kamadak-exif` | 0.6.1 | BSD 2-Clause | Yes |
| `blake3` | 1.8.7 | CC0 1.0, Apache 2.0, or Apache 2.0 with LLVM's exception | Yes |
| `xmp_toolkit` | 1.12.1 | MIT or Apache 2.0; it builds Adobe's XMP Toolkit, BSD 3-Clause, with expat and zlib (Photomatch's `docs/licences.md`) | Yes (unchecked for the toolkit's own terms beyond its notices) |
| `serde`, `serde_json` | 1.0.229, 1.0.154 | MIT or Apache 2.0 | Yes |
| `windows`, on Windows | 0.62.2 | MIT or Apache 2.0 (unchecked here) | Yes |

None is GPL or AGPL.

## Open questions

- Which of `ratings.rs` is the shared model and which is Photomatch's store? Step 1.4.
- Does Photomatch answer its thumbnail requests in a way that can abort, as PhotoDev's did? Step 2.1, in Photomatch's code.
- How Photomatch's CI fetches the private library ([0002](../spec/decisions/0002-one-rust-crate-in-its-own-repository-pinned-by-each-app.md)). Photomatch's installers are built by CI, so its next release waits on it.
