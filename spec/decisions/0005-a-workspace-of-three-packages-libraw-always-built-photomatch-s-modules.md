# 0005. A workspace of three packages, LibRaw always built, and PhotoMatch's modules as the API

- Status: Accepted
- Date: 2026-11-21

## Context

Plan step 1.1: the crate's skeleton, with the vendored `rsraw` and `rsraw-sys`, and LibRaw 0.22.3 as a submodule. Its design choices were laid out to the owner on 2026-11-20, each checked against Cargo's and Rust's own documentation (the owner's rule, in [`AGENTS.md`](../../AGENTS.md)):
- **How many packages.** The Cargo book's build-scripts page: a `-sys` package should "link to the native library" and "provide declarations for types and functions… but not higher-level abstractions"; "It is common to have a companion package without the `-sys` suffix that provides a safe, high-level abstractions on top of the sys package", as `git2` over `libgit2-sys`.
- **What an app fetches.** The Cargo book's page on specifying dependencies: Cargo "traverses the file tree to find `Cargo.toml` file for the requested crate anywhere inside the `git` repository", and "automatically fetches its submodules recursively".
- **LibRaw for an app that reads no RAW file,** such as PhotoBook today. The Cargo book's features page: an optional dependency "will only be included if the feature is enabled", and features "should be additive".
- **What the library shows.** The Rust reference on `non_exhaustive`: such an enum, outside its crate, needs a wildcard arm in a `match`, so a variant added later breaks no one. Rust's API guidelines, future-proofing: "Making a field public is a strong commitment".

And one fact found while building it, from the Cargo book's page on overriding dependencies: "Cargo only looks at the patch settings in the `Cargo.toml` manifest at the root of the workspace. Patch settings defined in dependencies will be ignored." PhotoMatch puts its vendored `rsraw` and `rsraw-sys` in place of the published ones with `[patch.crates-io]`; in the library, an app depending on it would never see that patch, and would build the published `rsraw-sys`, with LibRaw 0.21.3.

## Decision

The owner chose, on 2026-11-20, "as you said":
- **One workspace of three packages:** `photo-library` at the repository's root, and the vendored `rsraw` and `rsraw-sys` in `vendor/`, LibRaw 0.22.3 as `rsraw-sys`'s submodule, pinned at commit `cf02d66`, as PhotoMatch has it.
- **The vendored packages by path, not by `[patch]`:** `photo-library` depends on both by path, and the vendored `rsraw` depends on `rsraw-sys` by path, a change to its manifest marked as the library's. So an app gets LibRaw 0.22.3 with no patch of its own.
- **LibRaw always built** for now. An optional `raw` feature, on by default, comes in the step that moves PhotoBook, which reads no RAW file today.
- **PhotoMatch's modules as the API:** `photo`, `raw`, `image_file`, `os_image`, `thumbs`, `problem` and `files`, with `folder`, `ratings` and `sidecar` later; public what PhotoDev lists and PhotoMatch uses, the rest the crate's own. `Problem` is `#[non_exhaustive]`. Plain data structs keep their public fields.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Two packages, `rsraw` merged into `photo-library` | A rewrite of `rsraw`, against [0004](0004-photomatch-s-code-as-it-is-is-the-starting-point.md) |
| More packages, such as the sidecars apart | No app needs the split, and more versions to keep in step |
| The vendored packages through `[patch.crates-io]`, as PhotoMatch does | Ignored in a dependency: the apps would get LibRaw 0.21.3 |
| A `raw` feature now | No app needs it before PhotoBook moves |
| A curated set of names at the crate's root, the modules hidden | A rewrite of paths in both apps |

## Consequences

- **An app depends on `photo-library` alone,** by git at a commit, and gets `rsraw` and LibRaw with it; it drops its own `[patch.crates-io]` for them.
- **Building it needs** a C and C++ compiler, and libclang for `bindgen`, which `rsraw-sys` runs when the build is for the machine it runs on; on a Mac, Xcode's Command Line Tools. LibRaw takes about a minute the first time.
- **`Problem` being `#[non_exhaustive]`** asks a wildcard arm of an app that matches on it.

## Assumptions to verify

- [x] The workspace builds and its tests pass on the owner's Mac, LibRaw 0.22.3 linked: `cargo test --offline` on 2026-11-21, on the owner's Mac (Apple M3, macOS 14.3), 15.5 s from nothing, its one test passing, LibRaw's own version string starting `0.22.3`.
- [ ] An app builds with the library as a git dependency at a commit, LibRaw's submodule fetched with it: step 2.1.

## Sources

- The owner, on 2026-11-20: "as you said".
- The Cargo book: [build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html), [specifying dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html), [features](https://doc.rust-lang.org/cargo/reference/features.html), [overriding dependencies](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html), [workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html); the Rust reference, [`non_exhaustive`](https://doc.rust-lang.org/reference/attributes/type_system.html); Rust's API guidelines, [future-proofing](https://rust-lang.github.io/api-guidelines/future-proofing.html). All read on 2026-11-20.
- PhotoMatch's `app/src-tauri/Cargo.toml`, its `[patch.crates-io]`, at commit `391fd74`.
