# 0022. Canon's and Nikon's focus points, from their AF blocks; Nikon's named points by the Focus Points plugin's maps

- Status: Accepted
- Date: 2026-11-26

## Context

PhotoMatch's plan step 2.81, its record 0068. Record [0021](0021-the-camera-s-focus-point-from-its-maker-notes-and-whether-the-photo-is-in-focus-there.md) read Sony's, Fujifilm's and Olympus's focus points and left Canon's and Nikon's, which LibRaw keeps in blocks it doesn't decode (`makernotes.common.afdata`), to this step. PhotoMatch's owner, on 2026-11-26, of the choices laid out that day: "go with your recommendations".

What was found on 2026-11-26:
- **LibRaw 0.22.3 keeps the blocks** of all six Canon and Nikon samples: Canon's tag 0x0026, 546 bytes on the 5D Mark IV, 8,838 on the R5 Mark II and R6 Mark III; Nikon's 0x00b7, versions 0101 (D850), 0301 (Z 6II) and 0400 (Z 8), as 101, 301 and 400, each copy starting 4 bytes into ExifTool's layout, after the version; all little-endian.
- **ExifTool's documentation** ([Canon](https://exiftool.org/TagNames/Canon.html#AFInfo2), [Nikon](https://exiftool.org/TagNames/Nikon.html), read that day; its layouts, none of its code):
  - Canon's AFInfo2: 16-bit words, the AF areas' count, how many are valid, the AF image's size, each area's width, height, x and y, then bits for those in focus and those selected; AFInfo3, 0x003c, of the same layout. "0,0 is the image center. The direction of the Y axis depends on the camera model, with positive Y upwards for EOS models, but apparently downwards for PowerShot models."
  - Nikon's AFInfo2 by version: V0100 (D3, D4, D300, D700, D7000, D810 and others) and V0101 (D5, D500, D850, D7500, D3400, D3500) record the AF area only for contrast-detect AF, "center of AF area in AFImage coordinates", else a primary point by number; V0300 (Expeed 6: Z 5, Z 6, Z 6II, Z 7, Z 7II, Z 50, Z fc, D6, D780) and V0400 (Expeed 7: Z 8, Z 9, Z f, Z6III, Z50II, Z5II) say whether the area is recorded, `AFCoordinatesAvailable`, else the points used as bits, "either AFPointsUsed or AFAreaX/YPosition will be set, but not both". The point numbers, from the middle out, 51, 39, 153, 81 and 105 of them, listed in full.
- **The Focus Points plugin for Lightroom Classic** ([musselwhizzle/Focus-Points](https://github.com/musselwhizzle/Focus-Points), Apache 2.0, at `9e00882`, read that day): `CanonDelegates.lua` runs Canon's y upwards but on compacts; `NikonDelegates.lua` takes Nikon's recorded area as it is, and places named points by a map of each camera's points, 29 maps in `focus_points/nikon/`, 71 KB, each point's middle in the camera's image pixels, the image's size in a comment; `NikonDuplicates.lua` gives the cameras that share one, the Z 8 the Z 9's.
- **Drawn over the samples:** the R6 Mark III's area, its y upwards, on the photo's pinwheel, downwards in its flowers; the R5 Mark II's Spot AF on the bottle's sharp label, downwards on a soft white pot; the 5D Mark IV's middle area, in focus and selected, on a colour chart. The Z 6II's recorded area at the middle; the D850's primary point 1, E9, the middle of its map; the Z 8's 49 points used, in auto-area, a block of 7 by 7 on a grid of 27 columns, the Z 9 map's 15 rows by 27, its FocusResult 1, in focus. LibRaw can't unpack the Z 8's RAW data.

## Decision

**`focus::point` reads Canon's and Nikon's AF blocks (`af.rs`): the AF area where the camera records it, its own size kept; Nikon's named points placed by the Focus Points plugin's maps, built in unchanged with their Apache 2.0 licence (`src/af_maps/`); several areas or points as one box around them.**

- **Canon:** the valid areas in focus, else those selected; a box around them, in shares of the AF image; y upwards on an EOS, downwards on any other, as a PowerShot. None where no area is valid, as with manual focus.
- **Nikon, recorded:** V0300 and V0400 where `AFCoordinatesAvailable` is 1, V0100 and V0101 with contrast-detect AF: the area's middle and size over the AF image.
- **Nikon, named:** V0300's and V0400's points used, the bits running each row of the camera's map in turn, as A1 to A9 then B1 to B11, which the Z 8's block of 7 by 7 bears out on the Z 9's grid; where none is set, V0300's primary point. V0100's and V0101's primary point, by its number in ExifTool's list for the camera's grid (`af_numbers.rs`). A point between those a map lists, as the 153-point maps list only the 55 that can be chosen, between its neighbours by row and column.
- **The box's size:** the AF area's, or the points', where they give one; Sony's, Fujifilm's and Olympus's stay 6% of the short side.
- **A file LibRaw can't unpack,** as the Z 8's, keeps its point, `ThumbCache::focus_point` saying it can't tell there; the cache's name for focus points goes to version 2, so that Canon and Nikon files kept as having none are read again.
- **Not here:** Canon's older AFInfo (0x0012); the D780, whose two maps a field not read chooses; the 11-point cameras, with no map; a photo cropped in the camera, as a DX crop on an FX body, read as if it weren't.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Nikon's recorded areas alone, no maps | The Z 8 in auto-area and every DSLR through its viewfinder would show none |
| Maps made here, from the cameras' manuals | Their AF areas' places aren't given in pixels; the plugin's were measured by its users and are used by many |
| The primary point alone where several were used | The camera used them all; one box around them says so |
| Each point its own box | One verdict for one box is what PhotoMatch shows |
| The camera's own in-focus flag as the verdict | One answer, PhotoMatch's, the same for every maker; the R6 Mark III's pinwheel, its area not flagged in focus, is marked in focus |
| Canon's y one way for every camera | ExifTool and the plugin both find PowerShots' the other way |

## Consequences

- **71 KB of maps** built into the library, and their licence in PhotoMatch's third-party notices.
- **Nikon's named points are as right as the plugin's maps,** measured by its users; the bit order checked on the Z 8 alone, the primary point's numbering on the D850's middle point alone.
- **A large box,** as the Z 8's 21% by 36%, is in focus where any 1% of it is marked.

## Assumptions to verify

- [x] Canon's y runs upwards on an EOS: on 2026-11-26, the R5 Mark II's and R6 Mark III's areas on their subjects that way, on soft parts the other.
- [x] The maps read, each one's middle point at its image's middle: a test, all 29.
- [x] The tests pass: on 2026-11-26, 95, 99 with `places`, 4 new; the example over PhotoMatch's RAW samples: 5D Mark IV in focus, R6 Mark III in focus, R5 Mark II, D850 and Z 6II can't tell, the Z 8's box around its points; Sony's, Fujifilm's and Olympus's as before.
- [ ] A PowerShot's y downwards: no PowerShot sample has an AF block; the G1's has none.
- [ ] Nikon's named points on a photo focused off the middle: no sample has one.
- [ ] A photo cropped in the camera: no sample.

## Sources

- PhotoMatch's owner, on 2026-11-26.
- ExifTool's Canon and Nikon tag pages, read on 2026-11-26.
- The Focus Points plugin's `CanonDelegates.lua`, `NikonDelegates.lua`, `NikonDuplicates.lua` and maps, at `9e008821b9dd95c7c2b5bae655a29ed282fbf0d1`, read on 2026-11-26.
- `examples/focus_points.rs` over PhotoMatch's samples.
