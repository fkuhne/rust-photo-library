# 0002. One Rust crate in its own repository, pinned by each app

- Status: Proposed
- Date: 2026-11-20

## Context

Three apps share code ([`product.md`](../product.md#the-problem)). They were offered, in PhotoDev's repository on 2026-11-20:
- copying on, as PhotoDev's and PhotoBook's records 0008 chose;
- a shared library.

The owner chose the library. Each app is a Tauri 2 app with its own repository: `photomatch-dev`, `photodev-dev`, `photobook`. They're all private, and all on the owner's Mac under `~/Documents/claude/`. CI is off: the owner's GitHub Actions credits are spent.

## Decision

- **One crate, `photo-library`, in its own repository, `rust-photo-library`.** Private for now ([0003](0003-private-now-written-so-it-can-be-opened.md)). Its vendored `rsraw` and `rsraw-sys`, with LibRaw as a git submodule, live in it too.
- **Each app depends on it by git, at a pinned commit:** `photo-library = { git = "…/rust-photo-library", rev = "<commit>" }`. A new version reaches an app only when that app moves its pin, and passes its own tests.
- **While a change is under way,** an app points at the local folder through `[patch]` in its own workspace, uncommitted. So the change is built and tested in every app before the library's commit, and before any pin moves.
- **The order of a change:** made and tested in the library and in the apps on the owner's Mac; committed in the library; then each app's pin moved, in a commit of its own.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Copying on | The owner chose a library. The copies had begun to drift, and two apps now write the same sidecars |
| The library as a crate inside Photomatch's repository | The other apps would depend on Photomatch's whole repository, and its decisions would mix with Photomatch's |
| A git submodule of the library in each app | The apps would carry its whole source tree, and moving to a new version would mean moving the submodule; a git dependency pins the same commit with less in each repository |
| A path dependency, committed | It works only on a machine with the same folders, and never in CI |
| Published on crates.io now | It would be public, before the owner has chosen to open it |

## Consequences

- **One place to fix:** a fix in the library reaches each app when its pin moves.
- **The apps can move at their own pace:** Photomatch can stay on an older commit while PhotoDev moves on.
- **CI, when it runs again, needs credentials** to fetch a private git dependency: a deploy key or a token (unchecked how GitHub Actions does it). Until then, nothing is lost: CI is off.
- **A local `[patch]` must never be committed** in an app. Each app's `AGENTS.md` says so.

## Assumptions to verify

- [ ] Cargo builds an app with the library as a git dependency at a commit, LibRaw's submodule included. Step 2.1.
- [ ] A local `[patch]` overrides it while a change is under way. Step 2.1.

## Sources

- The owner, in PhotoDev's conversation of 2026-11-20.
- `../photodev/spec/decisions/0008-copy-from-photomatch-piece-by-piece-when-a-step-needs-it.md` and `0021-photodev-rates-filters-and-sorts-as-photomatch-does-sharing-the-ratings.md`.
