# 0007. One folder listing from both apps, hidden files and folders left out

- Status: Accepted
- Date: 2026-11-21

## Context

Plan step 1.3: folder listing, from PhotoDev's `folder.rs`. It can't move as it is: its entries carry an `edited` flag read from PhotoDev's edits files, which stay PhotoDev's ([`docs/inventory.md`](../../docs/inventory.md)), and its errors are PhotoDev's own type, not the library's `Problem`. PhotoMatch lists folders too, in its `lib.rs`, which the plan hadn't named. Read on 2026-11-21, PhotoMatch's at commit `391fd74` and PhotoDev's at `d95ff89`:

| | PhotoMatch | PhotoDev |
|---|---|---|
| The walk | `walkdir` | its own recursion with `read_dir` |
| Hidden files and folders, named with a dot first | listed | left out |
| A subfolder that can't be read | its path and why, for Diagnostics | skipped, silently |
| A folder that isn't there, or can't be listed | `FolderMissing`, `FolderNotAllowed`, `FolderUnreadable`, the library's kinds | `NotFound`, `Unreadable`, its own |
| Each photo | its path and its companion JPEG; PhotoMatch then adds its rating | path, companion, `raw`, `edited` |

`walkdir`'s own documentation (docs.rs, version 2.5.0, read on 2026-11-21): leaving out hidden files and directories, "without recursing into hidden directories", is its example of `filter_entry`; each entry comes as its own `Result`, so a folder that can't be read doesn't end the walk; symbolic links aren't followed, "default off"; its licence is Unlicense or MIT.

## Decision

The owner chose, on 2026-11-21, option A: **one listing, `folder::list`, made of both:**
- **PhotoMatch's walk,** with `walkdir`: the folder's problems as the library's kinds, and the paths that couldn't be opened reported beside the photos.
- **PhotoDev's `raw` flag,** on each photo.
- **PhotoDev's leaving out of hidden files and folders,** and what's inside a hidden folder, through `walkdir`'s `filter_entry`. The folder opened is listed even if its own name starts with a dot.
- **No `edited` flag:** PhotoDev sets it after listing, from its edits files.

The API: `list(folder) -> Result<Listing, Problem>`, a `Listing` being the photos (`Entry`: path, companion, `raw`), sorted by path, a RAW+JPEG pair as one photo, and the paths that couldn't be opened, each with why.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| PhotoDev's listing, adapted; PhotoMatch keeping its own | The listing would stay in two places |
| PhotoMatch's as it is, hidden files listed | PhotoDev would list what it leaves out today, such as the `._` copies macOS writes on memory cards |
| The `edited` flag in the library | PhotoDev's edits files stay PhotoDev's |

## Consequences

- **PhotoMatch stops listing hidden files and folders** when it moves onto the library, a change in its behaviour, recorded in its repository then (step 2.1). A `._DSC0001.ARW` that macOS writes beside a photo on a memory card, which PhotoMatch shows as a photo it can't read (unchecked), is no longer listed.
- **PhotoDev gains the report of what couldn't be read,** and the library's kinds for a folder's problems in place of its own (step 2.2).
- **`walkdir` comes in,** at PhotoMatch's version, 2.5.0.

## Assumptions to verify

- [ ] macOS writes `._` files beside photos copied to a memory card formatted in FAT32 or exFAT, which PhotoMatch lists today (unchecked).
- [ ] The listing gives PhotoMatch the same photos as before in its sample folders, none of which holds a hidden file: step 2.1. So far, the same counts: `cargo run --example list_folder` on 2026-11-21 listed 10, 15, 15, 2 and 4 photos in `sony-arw`, `other-makers`, `image-files`, `raw-jpeg-pair` and `broken-files`, one RAW+JPEG pair, nothing unopened, as PhotoMatch's own listing shows them; the paths themselves are compared in step 2.1.

## Sources

- The owner, on 2026-11-21: "A, go ahead".
- PhotoMatch's `app/src-tauri/src/lib.rs`, `list`, at commit `391fd74`; PhotoDev's `app/core/src/folder.rs` at commit `d95ff89`.
- [`walkdir` 2.5.0's documentation](https://docs.rs/walkdir/2.5.0/walkdir/), read on 2026-11-21.
