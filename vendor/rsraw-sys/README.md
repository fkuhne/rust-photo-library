# rsraw-sys, with LibRaw 0.22.3

A copy of [`rsraw-sys`](https://github.com/hexilee/rsraw) 0.1.1 (MIT, see `LICENSE`), which builds LibRaw from the submodule in `LibRaw/`, pinned to LibRaw's 0.22.3 tag, instead of the 0.21.3 the published crate bundles. Why: [record 0013](../../../../spec/decisions/0013-build-libraw-0-22-3-from-a-pinned-copy.md).

What differs from the published crate:
- `build.rs` compiles LibRaw 0.22's four new source files: `decoders/olympus14.cpp`, `decoders/pana8.cpp`, `decoders/sonycc.cpp` and `decompressors/losslessjpeg.cpp`.
- `src/bindings.rs`, used only when cross-compiling, is made again from LibRaw 0.22.3's headers.
- `glue/shot_details.cpp`, the library's own, compiled with LibRaw: a callback that keeps EXIF's SubSecTimeOriginal as LibRaw reads a file, which LibRaw doesn't keep, and `rsraw_shot_details` and `rsraw_watch_shot_details` in `src/lib.rs` (the library's [record 0023](../../spec/decisions/0023-the-time-s-milliseconds-the-drive-mode-and-the-camera-s-serial-number-for-finding-bursts.md)).

**To move to a newer LibRaw:** check out its tag in `LibRaw/`, add any new source files under `LibRaw/src/` to `build.rs`, build, copy the `bindings.rs` the build makes in its `OUT_DIR` to `src/`, and read every sample again.

A fresh clone needs `git submodule update --init` first.
