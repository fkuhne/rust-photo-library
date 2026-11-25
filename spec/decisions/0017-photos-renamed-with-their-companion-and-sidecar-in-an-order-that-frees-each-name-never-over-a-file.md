# 0017. Photos renamed with their companion and sidecar, in an order that frees each name, never over a file

- Status: Accepted
- Date: 2026-11-24

## Context

PhotoMatch's plan step 2.72 and its record 0057: at the photographer's word, photos renamed, one by a name typed or many by a pattern, and ⌘Z names them back. A photo's files are this library's to know (record 0015), and moving them, never over a file, is `moving`'s (record 0016). PhotoDev and PhotoBook would rename the same way.

What was known on 2026-11-24:
- **`moving`'s rename never writes over a file:** `renamex_np` with `RENAME_EXCL` on a Mac, `MoveFileExW` without its flags on Windows, a hard link elsewhere (record 0016). Before a move, `place` looks for a file at each new name.
- **On the owner's Mac, APFS, case-insensitive:** `renamex_np` with `RENAME_EXCL` renames `DSC0001.ARW` to `dsc0001.arw`, the same file, and refuses `B.ARW` to `C.ARW` with `C.ARW` there, `EEXIST`. A look for `dsc0001.arw` finds `DSC0001.ARW`, so a look first would refuse a change of case.
- **Photos renamed together may take each other's names:** a shoot's `_002` to `_101` renamed `_001` to `_100`, one name freed by the photo before. Renamed one after another in the wrong order, each would find its name taken.
- **`Ratings::moved`** takes each photo's entry out and puts it in at its new path, one pair after another; `ThumbCache::moved` renames each cache file in turn. With two photos swapping names, the first pair's entry would be written over by the second's.
- **`same-file` 1.0.6,** Unlicense or MIT: `is_same_file` "returns true if the two file paths may correspond to the same file" ([docs.rs](https://docs.rs/same-file/1.0.6/same_file/)), by their device and inode on Unix and their volume's serial number and file index on Windows (its `unix.rs` and `win.rs`). It's in the graph already, through `walkdir`.
- **FastRawViewer** renames a photo's JPEG, XMP and files of its base name with it, and "if any file in a group hits an error or conflict, the whole group returns to its original name" ([Renaming Files](https://www.fastrawviewer.com/node/767)).

## Decision

**`moving::rename` renames photos in their own folders, each with the JPEG shot beside it and its XMP sidecar, never over a file: in an order that frees each name first, through a hidden name where they take each other's. A change of case alone is a rename. `Ratings::moved` and `ThumbCache::moved` follow photos that swap names.**

- **`Renaming::to_stem(photo, companion, stem)`:** each of the photo's files, as `trashing::files_of` lists them, with `stem` before its own extension, in its folder: `DSC0001.ARW`, `.JPG` and `.xmp` as `Paris_001.ARW`, `.JPG` and `.xmp`. `Renaming::back(&Placed)` names them back.
- **`rename(&[Renaming], each)`:**
  - a photo whose new names are free is renamed, its files together, `rename_all` putting back those renamed if one can't be;
  - a name is free when no file has it, or the file there is one of the photo's own, its name in another case;
  - a name held by another photo of the run waits for that photo; when none can go, one photo held up by another of the run moves to hidden names, `.photomatch-<pid>-<n>.ARW`, and goes on from there once its names are free;
  - a name held by a file outside the run is refused, `NotRenamed`; a photo that went to hidden names goes back to its own, or, if another photo of the run took them meanwhile, to its own with the smallest number free, and says so;
  - `each` hears of every photo, a `Placed` with `Done::Moved`, which `rename` names back.
- **`taken(&[Renaming])`:** for each photo, whether a file outside the run has its new name, whatever its extension, each folder listed once: a RAW file renamed `Paris_001` beside a `Paris_001.jpg`, or a sidecar of that name, would take it as its own. For an app's preview.
- **`number_taken(&mut [Renaming])`:** Keep Both, a taken photo's new name with the smallest number from 2 free in its folder and in the run, `Paris-2`, its JPEG and sidecar alike.
- **`name_fault(stem)`:** whether a name is one both systems allow: not empty; none of `< > : " / \ | ? *` nor the characters 0 to 31; no space or period at the end; no period first, which hides it; not CON, PRN, AUX, NUL, COM1 to COM9 or LPT1 to LPT9, with their superscript digits, before a period or alone; at most 255 bytes with the longest extension of its files. As Microsoft's Naming Files says, and Apple's colon.
- **On a Mac,** where the file system doesn't support `RENAME_EXCL`, the look before the rename lets a file that is the photo's own through, so that a change of case goes.
- **`Ratings::moved` and `copied`** take every entry out before putting any in, and a photo moved to a path with an old entry and none of its own leaves it empty. **`ThumbCache::moved`** goes through hidden names when a file's new name is another's old one.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Every photo through a hidden name, then to its own | Twice the renames, and a run cut short leaves every photo hidden, where the order leaves at most one |
| Names held by the run refused, as names taken | A shoot's sequence couldn't be renamed one along |
| A look for each new name first, as `place` does | Refuses a change of case on a case-insensitive drive, a Mac's and Windows' own |
| The app's own order of renames | PhotoDev and PhotoBook would need the same, and the files are this library's |
| No check of names | A name with a colon, or `CON`, fails on Windows, and one beginning with a period vanishes from the listing |

## Consequences

- **`same-file` 1.0.6 becomes a direct dependency;** no package joins ([`docs/inventory.md`](../../docs/inventory.md)).
- **`Ratings::moved` and `ThumbCache::moved` hold for photos that swap names,** for moving too.
- **A run cut short,** by the app stopped, may leave one photo under a hidden name, `.photomatch-…`, which the listing leaves out. Its files are whole.

## Assumptions to verify

- [x] On a Mac: a photo renamed with its JPEG and sidecar, and back; a change of case alone; photos swapping names, and a sequence one along; a name taken refused, whatever its extension, and Keep Both numbering it; the ratings and the thumbnails following photos that swap names: this library's tests, on the owner's Mac. On 2026-11-24, 73 pass, 77 with `places`, 7 new; the two for the ratings and the thumbnails fail on the code before this step, and pass on it.
- [ ] On Windows, `MoveFileExW` renames a change of case alone, and `same-file` tells it the same file: PhotoMatch's check on the owner's PC.

## Sources

- PhotoMatch's owner, on 2026-11-24; PhotoMatch's record 0057.
- `renamex_np` tried on the owner's Mac; `same-file` 1.0.6's documentation; Microsoft's Naming Files, Paths, and Namespaces; FastRawViewer's Renaming Files; read on 2026-11-24.
