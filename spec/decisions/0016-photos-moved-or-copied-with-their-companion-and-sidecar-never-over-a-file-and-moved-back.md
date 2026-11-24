# 0016. Photos moved or copied with their companion and sidecar, never over a file, and moved back

- Status: Accepted
- Date: 2026-11-24

## Context

PhotoMatch's plan step 2.71, and its record 0056: at the photographer's word, photos move or copy to another folder, from File → Move to and Copy to, or a drag onto a folder of its Folders panel, and ⌘Z moves them back. A photo's files are this library's to know (record 0015): the RAW file, the JPEG shot beside it, and the XMP sidecar. Its ratings and tags are kept by its path (`ratings`), and its thumbnails by a key made of its path, size and time of change (`thumbs`). PhotoDev and PhotoBook would need the same.

What was known on 2026-11-24:
- **Rust's `std::fs::rename`** replaces `to` if it exists, and fails when the two are "on separate filesystems"; `std::fs::copy` "will overwrite the contents of `to`" ([rename](https://doc.rust-lang.org/std/fs/fn.rename.html), [copy](https://doc.rust-lang.org/std/fs/fn.copy.html)). PhotoMatch's record 0004 never writes over a photo.
- **On a Mac,** `renamex_np` with `RENAME_EXCL` returns `EEXIST` "if the destination already exists", on file systems that support it, and `ENOTSUP` where they don't; `EXDEV` across devices (its manual page). `libc` 0.2.191, MIT or Apache 2.0, already in the library's graph, has it.
- **On Windows,** `MoveFileExW` replaces a file only with `MOVEFILE_REPLACE_EXISTING`, and moves to another drive only with `MOVEFILE_COPY_ALLOWED` ([Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)); the `windows` crate 0.62 has it, under `Win32_Storage_FileSystem`.
- **Apple and Windows** move on the same drive and copy to another by default ([Apple's Drag and drop](https://developer.apple.com/design/human-interface-guidelines/drag-and-drop); Windows' defaults, from Microsoft Q&A, as a search quoted it).

## Decision

**`moving::place` moves or copies a photo's files into a folder, together, never over a file there; to another drive it copies them, checks each byte for byte, and sends the originals to the Trash; `moving::put_back` undoes it. `Ratings::moved` and `copied` and `ThumbCache::moved` let an app move what it keeps by the photo's path.**

- **A photo at a time,** its files together, as `trashing::files_of` lists them: if one can't go, those gone come back, or the copies made go, and the photo stays whole.
- **Never over a file:** a name taken is refused, `NotMoved` or `NotCopied`. `clashes` says so beforehand, and `free_number` gives the smallest number from 2 free for all the photo's files, for an app's "Keep Both": `DSC0001-2.ARW`, its JPEG and sidecar alike.
- **On the same drive,** each file renamed:
  - on a Mac by `renamex_np` with `RENAME_EXCL`, where the file system doesn't support it, a look first, then a rename;
  - on Windows by `MoveFileExW` without its two flags;
  - elsewhere, a hard link then the old name removed.
- **To another drive,** as `same_drive` tells, by the device on a Mac and the drive or share on Windows; or when a rename says the files cross devices:
  - each file copied under a hidden name beside its new one, its time of change kept, written through, then checked by a hash of both, then given its name, never over a file;
  - a move then sends the originals to the Trash (record 0015), never deletes them; where they can't go, a drive without a Trash, they stay, and `Done::OriginalsKept` says why.
- **Undo** renames a move back; brings a move across back from the Trash and sends its copies there; sends a copy's copies to the Trash. Nothing deleted outright, but a hidden part of a copy that failed, which is the library's own.
- **The ratings and tags** move with `Ratings::moved`, or a copy gets them with `copied`, saved; the sidecars moved with the photos keep their stamps.
- **The thumbnails and details** move with `ThumbCache::moved`, renamed to the new path's key, or copied for a copy, the cache listed once for them all.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| `std::fs::rename` after a look | A file arriving between the look and the rename would be written over |
| A hard link everywhere | Camera cards' FAT and exFAT have none |
| `std::fs::copy` straight to the name | A file there would be written over, and a copy cut short would carry the photo's name |
| Deleting the originals after a move across | The first outright delete; the Trash keeps them, as PhotoMatch's record 0053 asks |
| No check of the copies | A copy gone wrong would only show when the originals are gone |

## Consequences

- **`libc` becomes a direct dependency on a Mac,** at the version the graph had; `windows` gains a feature. No package joins ([`docs/inventory.md`](../../docs/inventory.md)).
- **A move across reads each file twice more,** for its hash and its copy's: computed, a 50 MB RAW file 100 MB more read; the thumbnails aren't made again.
- **An app finishes its sidecar writes first,** as for the Trash, and moves its own records of the photos' places.

## Assumptions to verify

- [x] On a Mac, a photo moves with its JPEG and sidecar, and back; a name taken is never written over; Keep Both numbers all three; a copy keeps its time; and, to a disk image as another drive, the copies are checked, the originals go to the Trash, and come back. On 2026-11-24, on the owner's Mac: the five tests of `moving`, the last with `PHOTO_LIBRARY_OTHER_DRIVE` naming a 64 MB APFS disk image; 66 tests pass, 70 with `places`.
- [ ] On Windows, `MoveFileExW` refuses a name taken and another drive, and the copies across keep their times: PhotoMatch's check on the owner's PC. The Windows half type-checked for Windows on the Mac.
- [ ] A drive mounted in a folder on Windows is taken for the same drive, and the rename's `ERROR_NOT_SAME_DEVICE` then turns it into a copy (unchecked).

## Sources

- PhotoMatch's owner, on 2026-11-24; PhotoMatch's record 0056.
- Rust's `fs::rename` and `fs::copy`; `renamex_np(2)`; Microsoft's MoveFileExW; Apple's Drag and drop; read on 2026-11-24.
