# 0008. Ratings: the model, the store and the sidecars move whole

- Status: Accepted
- Date: 2026-11-21

## Context

Plan step 1.4: ratings and XMP sidecars, moved from PhotoMatch, and what stays PhotoMatch's own store decided. Read on 2026-11-21, at PhotoMatch's commit `391fd74`:
- **`ratings.rs`, 769 lines,** is two things: the ratings model (`Pick`, `Label`, `Rating`, `RatingChange`, `Changed`), and PhotoMatch's store, `Ratings`, a file in the app's data folder kept in step with the sidecars. The rules that make two apps safe on one sidecar live in the store: the newer change wins; a sidecar another app changed since wins; a failed write is tried again at the next sync. 14 tests.
- **`sidecar.rs`, 260 lines,** reads and writes a RAW file's XMP sidecar by PhotoMatch's record 0014, through `xmp_toolkit`: only the pick, the stars and the label are changed, and a sidecar it can't read is never written over. 5 tests, one of them that Lightroom's own fields are kept.
- **PhotoDev will rate** as PhotoMatch does, sharing a RAW file's ratings through its sidecar (the library's [`product.md`](../product.md), PhotoDev's record 0021); it has no ratings code yet.

Checked on 2026-11-21, as the owner asks before any recommendation:
- **The XMP specification, Part 3, Storage in Files:** "For applications that need to find external XMP files, look in the same directory for a file with the same name as the main document but with an .xmp extension. (This is called a sidecar XMP file.)", as PhotoMatch names them; and an application that modifies XMP must "preserve" what it finds of other formats.
- **Part 2, Additional Properties,** lists `xmp:Rating`, `xmp:Label` and `xmpDM:good`, and no `xmpDM:pick`: the pick follows Lightroom's own practice, as PhotoMatch's record 0014 says (unchecked there). The values of `xmp:Rating`, -1 for rejected and 0 to 5, are from a search's summary of the specification, as the PDF's tables didn't come out as text (unchecked).
- **`xmp_toolkit` 1.12.1's documentation:** it builds Adobe's C++ XMP Toolkit, needs Rust 1.88 or newer, and only Microsoft's compiler on Windows; MIT or Apache 2.0.

## Decision

The owner chose, on 2026-11-21, "as you said":
- **Everything moves:** the model, the store and the sidecars, `ratings.rs` and `sidecar.rs` as they are, with their 19 tests. Each app opens its own store, `Ratings::open`, at its own file in its data folder. Nothing of it stays PhotoMatch's but where its file is, and its interface's undo.
- **An image file's rating stays with the app that gave it:** it has no sidecar (record 0014: Lightroom reads a JPEG's own XMP, and the originals are never written), so PhotoMatch's and PhotoDev's ratings of a JPEG don't travel between them.
- **Two apps writing one sidecar at the same instant** is accepted: each write is atomic, so a sidecar is never half written, and the later write may undo the other's change. Rare; an assumption to verify.
- **`xmp_toolkit` is always built** for now; an optional feature, for an app that rates nothing, comes with PhotoBook's move, as LibRaw's ([0005](0005-a-workspace-of-three-packages-libraw-always-built-photomatch-s-modules.md)).
- **A new test:** a sidecar keeps another app's own fields, besides Lightroom's.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| The model and the sidecars only, each app keeping its own store | The rules that keep two apps safe on one sidecar would be written twice, the drift the library exists to end |
| One ratings file shared by all the apps | Two apps writing one file, and one losing the other's changes |
| A lock file beside a sidecar while it's written | More code, and a lock left behind by a crash |
| `xmp_toolkit` optional now | No app needs it before PhotoBook moves |

## Consequences

- **PhotoMatch's ratings file and its format don't change,** nor its sidecars: it opens the library's store at the same path (step 2.1).
- **PhotoDev rates by the very code PhotoMatch does,** opening a store of its own (step 2.2).
- **Building the library compiles Adobe's XMP Toolkit,** as building PhotoMatch did.

## Assumptions to verify

- [ ] Two apps rating one photo at the same instant is rare enough that the later write undoing the other's change doesn't matter: the owner, using both.
- [ ] PhotoMatch's sidecars come out the same on the library: step 2.1.

## What the move measured

`cargo test --offline` on the owner's Mac (Apple M3), on 2026-11-21: the 14 tests of `ratings.rs`, the 5 of `sidecar.rs` and the new one pass, with the library's 14 others and the 2 from outside; 15.8 s, Adobe's XMP Toolkit included. `xmp_toolkit` brought 15 packages from crates.io, every one MIT, Apache 2.0 or BSD 3-Clause; two of them, `toml_datetime` and `winnow`, were set back to PhotoMatch's versions ([0006](0006-dependencies-as-default-requirements-the-lockfile-for-the-library-s-own-builds.md)).

## Sources

- The owner, on 2026-11-21: "as you said, go ahead".
- PhotoMatch's `app/src-tauri/src/ratings.rs` and `sidecar.rs` at commit `391fd74`; its record 0014.
- [XMP Specification Part 3](https://raw.githubusercontent.com/adobe/XMP-Toolkit-SDK/main/docs/XMPSpecificationPart3.pdf) and [Part 2](https://raw.githubusercontent.com/adobe/XMP-Toolkit-SDK/main/docs/XMPSpecificationPart2.pdf), from Adobe's XMP-Toolkit-SDK repository, their text read with PDFKit on 2026-11-21; [`xmp_toolkit` 1.12.1's documentation](https://docs.rs/xmp_toolkit/1.12.1/xmp_toolkit/).
