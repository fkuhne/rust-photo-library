# 0015. Photos moved to the Trash with their companion and sidecar, and put back

- Status: Accepted
- Date: 2026-11-24

## Context

PhotoMatch's plan step 2.69, and its record 0053: at the photographer's word, after asking, photos go to the Trash, the Recycle Bin on Windows, and ⌘Z puts them back. A photo's files are this library's to know: the RAW file, the JPEG shot beside it (`folder::list`'s companion), and the XMP sidecar (`sidecar::path_for`). PhotoDev and PhotoBook would need the same.

What was known on 2026-11-24:
- **The `trash` crate,** 5.4.0, MIT ([docs.rs](https://docs.rs/trash/latest/trash/); its `Cargo.toml`, `lib.rs`, `macos/mod.rs` and `windows.rs` at its tag `v5.4.0` on GitHub):
  - on macOS, by Finder by default, which "Might ask the user to give additional permissions to the app"; or by `NSFileManager`'s `trashItemAtURL`, which asks none but may not show Finder's Put Back, "a macOS bug";
  - `os_limited::list` and `restore_all` on Windows and Linux only; `restore_all` refuses when a file is back at its place;
  - on Windows, `IFileOperation` with `FOF_ALLOWUNDO`, COM set up per thread in the apartment its feature names, `coinit_apartmentthreaded` by default, and a panic if the thread has another;
  - the `windows` crate at 0.62, as this library has.
- **Microsoft:** `IFileOperation` "can only be applied in a single-threaded apartment (STA) situation" ([IFileOperation](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation)). This library's `os_image` sets its threads to the multithreaded apartment, and an app runs both on a shared pool of threads.
- **`NSFileManager`'s `trashItemAtURL:resultingItemURL:error:`** gives the place the file took in the Trash; `objc2-foundation` 0.3.2, which the `trash` crate and Tauri use, has it.

## Decision

**`trashing::to_trash` moves a photo's files to the Trash, the photo, its companion and its sidecar, and says where each went; `trashing::put_back` moves them back.**

- **A photo at a time,** so that one that can't go doesn't hold the others back; its files together, and if one fails, those gone come back.
- **On a Mac,** through `NSFileManager`, which asks no permission, and gives each file's place in the Trash; put back by moving each from there.
- **On Windows and Linux,** through the `trash` crate's `delete_all`; put back by its `restore_all`, the items found by their original path, the newest. On Windows, each call runs on a thread of its own, single-threaded, as `IFileOperation` needs, whatever thread the app calls from.
- **A file back at its place by other means** is left as it is; the others come back.
- **What can't go, or come back,** is a problem of its own kind, `NotTrashed` or `NotPutBack`, with the photo's name and the cause.
- **Nothing here deletes a file outright,** nor touches the ratings, which an app keeps by the photo's path for its return.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| The `trash` crate on a Mac too | Its Finder way asks a permission; its `NSFileManager` way doesn't give the place in the Trash to put a file back from |
| All the photos in one call | One file refused would leave the others unsaid; Windows' `delete_all` names only the first that failed |
| The `trash` crate on the thread the app calls from | The `IFileOperation` panic when `os_image` set the thread to the multithreaded apartment |
| Moving the files to a folder of the app's | Not the Trash the photographer knows; space never given back |

## Consequences

- **The `trash` crate** joins the Windows and Linux builds, without a second `windows` crate; `objc2` and `objc2-foundation` the Mac build.
- **A thread made for each call on Windows,** a few milliseconds each (unchecked), for what a photographer does now and then.
- **An app finishes its sidecar writes first,** so that none comes back beside a photo gone.

## Assumptions to verify

- [x] On a Mac, a file moved to the Trash comes back by a move, without a permission asked. On 2026-11-24, on the owner's Mac, `a_photo_goes_to_the_trash_with_its_jpeg_and_sidecar_and_comes_back` passed from a terminal that isn't allowed to list `~/.Trash`; and PhotoMatch's self-check sent copies there and back in eight folders. An app opened from Finder: PhotoMatch's owner's look.
- [ ] On Windows, the files reach the Recycle Bin and come back: PhotoMatch's check on the owner's PC.

## Sources

- PhotoMatch's owner, on 2026-11-24; PhotoMatch's record 0053.
- The `trash` crate's documentation and code; Microsoft's IFileOperation; read on 2026-11-24.
