# Plan

The work, as small steps in order. Each step names the decision records it depends on or will create; see the [decision index](decisions/README.md). Each step ends in a few small commits.

**How we work through it**
- One step at a time. Before a step with design choices, the agent lays out the options, the evidence and a recommendation; the owner picks.
- Every dependency states its licence, and whether it allows MIT or Apache 2.0 publishing and closed use ([0003](decisions/0003-private-now-written-so-it-can-be-opened.md)).
- A new decision gets its record first, in its own commit. A Proposed record becomes Accepted in the commit that implements it.
- Guesses go into the matching record's "Assumptions to verify", ticked with evidence once checked.
- Tick the step here in the commit that finishes it.
- Built and tested on the owner's Mac; CI waits for credits.

**What a step leaves behind.** A step that measures something writes down the number, the command that produced it, and the settings and machine it ran on, in `docs/`. A number that lives only in a terminal is lost.

**Moved, not rewritten.** Code comes in from PhotoMatch as it is, with its tests ([0004](decisions/0004-photomatch-s-code-as-it-is-is-the-starting-point.md)). An app loses its copy in the same step that points it at the library.

## Phase 0: The spec

- [x] 0.1 The product spec, the requirements, this plan and the first records ([0001](decisions/0001-work-from-a-spec-and-record-every-decision.md) to [0004](decisions/0004-photomatch-s-code-as-it-is-is-the-starting-point.md)). Also a first pass of the inventory, [`docs/inventory.md`](../docs/inventory.md).
- [x] 0.2 The owner reviews the spec, the records and the inventory, and the first commit is made. The private GitHub repository `rust-photo-library` too, at the owner's word.

  Done on 2026-11-20: reviewed with the owner, who kept the crate's name `photo-library` and the repository's `rust-photo-library`, as Rust's API guidelines ask; the review's notes went into the inventory and step 2.1; first commit `846ce26`. The GitHub repository waits for the owner's word.

## Phase 1: The library

- [x] 1.1 The crate's skeleton, `photo-library`, in a Cargo workspace with the vendored `rsraw` and `rsraw-sys`, and LibRaw 0.22.3 as a submodule. It builds and its tests run on the owner's Mac. Licences listed in the inventory. *Design choices: one crate or several, such as LibRaw's apart, and what each module exposes; new record.*

  Done on 2026-11-21 ([0005](decisions/0005-a-workspace-of-three-packages-libraw-always-built-photomatch-s-modules.md) accepted): a workspace of `photo-library` and the vendored `rsraw` and `rsraw-sys`, copied from PhotoMatch unchanged but for one marked line, `rsraw`'s dependency on `rsraw-sys` by path; LibRaw as a submodule at `cf02d66`, 0.22.3. `cargo test` built it and passed its first test, LibRaw 0.22.3 linked, in 15.5 s on the owner's Mac (Apple M3), with PhotoMatch's dependency versions; 63 packages from crates.io, every licence listed in [`docs/inventory.md`](../docs/inventory.md). Disk: 234 MB of build files in `target/`, 3.3 MB of LibRaw's sources, and 17 MB in `.git`.
- [x] 1.2 Photo reading, thumbnails and problems, moved from PhotoMatch at its commit then, with their tests; `rsraw` merged from the two copies, each addition marked by whose it was; PhotoDev's one visible function. PhotoMatch's tests pass in the library.

  Done on 2026-11-21: PhotoMatch's six files at its commit `391fd74`, with `files.rs`, which `thumbs.rs` writes through, brought forward from step 1.3; `rsraw` merged, PhotoMatch's with PhotoDev's eleven functions, as PhotoDev's copy was older than PhotoMatch's; `upright`, `decode` and `encode` made public for PhotoDev; `Problem` non-exhaustive; `rsraw` re-exported; dependencies at PhotoMatch's versions ([0006](decisions/0006-dependencies-as-default-requirements-the-lockfile-for-the-library-s-own-builds.md)). `cargo test` on the owner's Mac: PhotoMatch's 9 tests and the skeleton's pass, and 2 more reach the library from outside, as an app does. See [`docs/inventory.md`](../docs/inventory.md#what-moved-and-what-changed-on-the-way).
- [ ] 1.3 Folder listing: PhotoDev's `folder.rs`, with its tests. PhotoMatch's `files.rs` came in step 1.2, as thumbnails need it.
- [ ] 1.4 Ratings and XMP sidecars, moved from PhotoMatch: the ratings model, and the sidecar rules of PhotoMatch's record 0014. What stays as PhotoMatch's own store is decided here. A test that a sidecar keeps Lightroom's fields, and another app's. *Design choices; new record.*

## Phase 2: The apps on it

- [ ] 2.1 PhotoMatch on the library: a git dependency at a commit, and its copies removed, in PhotoMatch's repository, with a record there. Its tests and self-check pass on the owner's Mac. A sample folder's thumbnails and sidecars come out the same as on its copy. Its thumbnail answers checked for PhotoDev's crash. What in PhotoMatch points at the vendored LibRaw follows it into Cargo's checkout of the library: `.gitmodules`; `app/scripts/notices.mjs`, which reads LibRaw's licence files; and `app/scripts/publish-release.sh`, which ships LibRaw's source with each release, as its CDDL asks. Its CI's access to the private library is needed before its next release. Flips [0002](decisions/0002-one-rust-crate-in-its-own-repository-pinned-by-each-app.md) and [0004](decisions/0004-photomatch-s-code-as-it-is-is-the-starting-point.md) to Accepted.
- [ ] 2.2 PhotoDev on the library: its `library` module and `vendor` removed, in PhotoDev's repository, with a record there that supersedes its 0008 for this code. Its tests, self-checks and originals test pass. Flips [0003](decisions/0003-private-now-written-so-it-can-be-opened.md) to Accepted, once every licence is listed.
