# 0023. The time's milliseconds, the drive mode and the camera's serial number, for finding bursts

- Status: Accepted
- Date: 2026-11-27

## Context

PhotoMatch's plan step 2.76, its record 0069: bursts found from the photos' metadata, frames of one camera a fraction of a second apart, or shot in its continuous drive mode. PhotoMatch's owner, on 2026-11-27, of the choices laid out that day: "go with your recommendations". `PhotoInfo` held the time a photo was taken to the second only, as LibRaw keeps it (`other.timestamp`, a `time_t`), and nothing of how the shutter was released or of which body took it.

What was found on 2026-11-27, over PhotoMatch's samples:
- **The fraction of a second:** EXIF's SubSecTimeOriginal, 0x9291, "fractional seconds for DateTimeOriginal" ([ExifTool's EXIF tags](https://exiftool.org/TagNames/EXIF.html)). LibRaw reads past it and keeps nothing; `kamadak-exif`, reading a whole file, found it in 7 of the 10 Sony bodies, the 5D Mark IV, the three Nikons, the K-3 Mark III and the phones' DNG files, 10 to 45 ms a RAW file, and couldn't read CR3, RAF, ORF or RW2 at all. LibRaw passes each tag of the EXIF directory, in every format it reads, to a callback it calls with its stream at the tag's value (`exif_gps.cpp`, `parse_exif`); the stream is a C++ object.
- **The drive mode,** kept by LibRaw 0.22.3 in the maker notes' fields, its values from [ExifTool's tag pages](https://exiftool.org/TagNames/), read that day: Sony's ReleaseMode2 in its 0x9400 block ([Sony](https://exiftool.org/TagNames/Sony.html), revised 22 May 2026), Canon's ContinuousDrive, Nikon's ShootingMode bit 0 ([Nikon](https://exiftool.org/TagNames/Nikon.html), revised 15 Nov 2026), Fujifilm's DriveMode ([FujiFilm](https://exiftool.org/TagNames/FujiFilm.html), revised 11 Aug 2026), Pentax's DriveMode. Olympus's DriveMode, 0x0600, LibRaw keeps, but ExifTool works its values out in code rather than listing them ([Olympus](https://exiftool.org/TagNames/Olympus.html), revised 3 Nov 2026). Panasonic's BurstMode, 0x002a ([Panasonic](https://exiftool.org/TagNames/Panasonic.html), revised 13 Apr 2026), LibRaw doesn't keep, and its maker note never reached LibRaw's maker-note callback on the GH7's or the S5 II's files.
- **The samples' drive modes:** the A6400's 26, "Continuous Low", the A7S III's 1, "Continuous", the K-3 Mark III's 1, "Continuous"; every other one single.
- **The serial number:** LibRaw's `shootinginfo.BodySerial`, else `InternalBodySerial`, recorded by every RAW sample's body but the RX100 VII.

## Decision

**`PhotoInfo` gains `captured_ms`, the milliseconds of the time taken; `drive`, single or continuous, where the file says so; and `serial`, the body's serial number. LibRaw's EXIF callback keeps SubSecTimeOriginal as it reads the file, through a small C++ function of the library's own in its copy of `rsraw-sys`; image files take it from their EXIF.**

- **The callback** (`vendor/rsraw-sys/glue/shot_details.cpp`): set by `rsraw::RawImage` before every file is opened, filling a buffer the image owns until LibRaw's handle is closed; `RawImage::subsec_time_original` reads it. `build.rs` compiles it with LibRaw and watches it, as bindgen's callbacks name only the headers.
- **Milliseconds** (`drive::milliseconds`): the digits after the second's point, "83" 830 ms, "669261" 669; for an image file, SubSecTimeOriginal with DateTimeOriginal, else SubSecTime with DateTime.
- **The drive mode** (`drive.rs`), by maker: Sony's ReleaseMode2 where its 0x9400 block was deciphered, its continuous values 1, 2, 5, 17, 19, 26, 27 and 29, its single ones 0, 3, 6, 18, 23, 28 and 146, and panoramas, HDR and Multi Frame NR, which the camera makes into one photo, not said; Canon's ContinuousDrive, 2, Movie, not said; Nikon's ShootingMode bit 0; Fujifilm's DriveMode, -1 not said; Pentax's and Ricoh's DriveMode's first byte, 255, Video, not said. Olympus, OM System and Panasonic not said; their milliseconds tell their bursts. Exposure brackets shot continuously are continuous.
- **The serial:** the body's, else the one inside it; an image file's BodySerialNumber.
- **`DETAILS_VERSION` 2,** so that details cached before are read again once, the thumbnails kept, as for the position (record 0010).

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| `kamadak-exif` over the RAW file | It read the whole file, 10 to 45 ms each, and none of CR3, RAF, ORF or RW2 |
| A patch to LibRaw's sources, to keep SubSecTimeOriginal in its own data | A change to LibRaw that each new version would need again; the callback is LibRaw's own API |
| The callback in Rust | It can't read LibRaw's C++ stream |
| Panasonic's BurstMode through the maker-note callback | Its maker note never reached the callback on the two samples |
| Olympus's DriveMode | Its values aren't listed; read wrong, a single frame would join a burst |

## Consequences

- **Every RAW file's details read once more** after this version reaches an app, in the background, as for the position; 0 to 2 ms a file over the samples, LibRaw's open of each.
- **CR3, RAF and RW2 files have their milliseconds** too, which `kamadak-exif` couldn't give: the R5 Mark II's .350, the X100VI's .270, the GH7's .617.
- **Three sub-second-less Sony bodies, the RX100 VII, A6400 and A7 III,** and the X-T5, the Leicas, the Ricoh, the OM-1 and older formats give whole seconds; for those, an app has the drive mode where it's read.

## Assumptions to verify

- [x] The milliseconds and drive modes as `kamadak-exif` and LibRaw's fields gave them: on 2026-11-27, `examples/shot_details.rs` over PhotoMatch's samples, the same, and CR3, RAF and RW2 too.
- [x] The tests pass: on 2026-11-27, 97, 101 with `places`, 2 new.
- [ ] A real burst's frames continuous, with their milliseconds, on Sony, Canon, Nikon and Fujifilm: no sample is a burst.
- [ ] Nikon's Z bodies set ShootingMode's bit 0 in continuous drive: the samples are single.
- [ ] Windows: the glue compiled by MSVC, at PhotoMatch's next Windows build.

## Sources

- PhotoMatch's owner, on 2026-11-27.
- ExifTool's EXIF, Sony, Canon, Nikon, FujiFilm, Olympus, Pentax and Panasonic tag pages, read on 2026-11-27.
- LibRaw 0.22.3's `exif_gps.cpp`, `makernotes.cpp`, `libraw_types.h` and `libraw_c_api.cpp`.
- `examples/shot_details.rs` over PhotoMatch's samples.
