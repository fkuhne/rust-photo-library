# 0021. The camera's focus point, from its maker notes, and whether the photo is in focus there

- Status: Accepted
- Date: 2026-11-26

## Context

PhotoMatch's plan step 2.80, its record 0064. The owner, on 2026-11-26: "I need to get the focus point of photos", and, of the choices laid out that day, "go with your recommendations". A RAW file records where the camera focused, each maker its own way; LibRaw reads some of it.

What was found on 2026-11-26, with `examples/focus_points.rs` over PhotoMatch's RAW samples, each point drawn over the photo and looked at:
- **Sony:** LibRaw's `makernotes.sony.FocusLocation`, four numbers, the frame's width and height and the point's x and y (ExifTool's Sony tag 0x2027, `int16u[4]`, without notes; the Focus Points plugin for Lightroom Classic reads them so, Apache 2.0). 11 of 13 Sony samples have one, the DSC-F828's and DSC-R1's zeros: the α6400's on its stone frog's face, where focus peaking marks it sharp; the ZV-E10's on the cactus's face, against its 16:9 frame of 6000 × 3376; the α7 IV's in its foliage, the α7R V's on its lawn. The A1's, A6700's, A7 III's and A9 III's are exactly at the frame's middle, real or a default, which a user on ExifTool's forum found an α7 IV's always to be.
- **Fujifilm:** `makernotes.fuji.FocusPixel`, x and y (ExifTool's tag 0x1023, `int16u[2]`), "the dimensions of the embedded JPEG image" being their frame, as the Focus Points plugin reads them: 4416 × 2944 on the X-T5 and X100VI; the X-T5's at the middle, its AF mode a single point, the X100VI's on its church's corner, turned with the photo.
- **Olympus:** `makernotes.olympus.AFPointSelected`, five numbers, "coordinates expressed as a percent" (ExifTool's tag 0x0305): the E-M5 II's 0, 0.498, 0.498, 0.498, 0.498, a box of no size at the middle; the OM-1's all zeros.
- **Canon and Nikon** keep theirs in blocks LibRaw leaves undecoded, `afdata`, ExifTool's AFInfo2: PhotoMatch's next step. Pentax's and Ricoh's samples say none was recorded; Panasonic's, Leica's, the phones' and the older cameras' have none LibRaw reads.
- **Turned:** LibRaw's `flip`, 3, 5 or 6, turns the point as the photo, as the marks.

## Decision

**`focus::point` reads where the camera focused, as a box in fractions of the upright image: Sony's `FocusLocation` over its own frame, Fujifilm's `FocusPixel` over its embedded JPEG's size, Olympus's `AFPointSelected` as its share of the frame; `None` where none is recorded. `focus::judge` says whether the photo is in focus inside the box, from the focus marks: in focus, not in focus, or can't tell.**

- **The box:** 6% of the image's short side, square, at the point, as no file read here gives its AF area's size.
- **In focus here:** at least 1% of the box's pixels marked, at the level asked.
- **Not in focus here:** fewer marked, and its detail plainly soft: at least 3% of its steps with detail over the noise but no steeper close up than three steps out, and steep steps fewer than a quarter of those.
- **Can't tell here:** neither, as a plain wall or sky, a lone straight edge the crossing rule leaves unmarked, or detail lost in near-black noise. A first version, "not in focus" wherever there was detail and no marks, put that on an α6700's point in deep shade inside a tree and an α7S III's on a near-black treetop against the sky, both likely in focus: unmarked is not soft.
- **Unrecorded:** a point of zeros, or one outside its frame, is none.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Canon and Nikon now | Their blocks need decoding by ExifTool's layouts, by version and, for Nikon's DSLRs, each model's grid of AF points: a step of its own |
| A point at the middle left out, as maybe a default | Many photos are focused there; the verdict tells whether it's sharp |
| The camera's own AF area size | Not in what LibRaw reads for these makers |

## Consequences

- **Reading the point** takes LibRaw's opening of the file, no unpacking; judging it, the focus marks' work, 57 ms to about a second.
- **The thresholds of the verdict** are set on the samples, the points drawn and looked at.

## Assumptions to verify

- [x] The verdicts match what's seen over the samples' points: on 2026-11-26, 15 points drawn and looked at, 12 in focus, as their subjects are; 3 can't tell, the α6700's in deep shade, the α7S III's on a near-black treetop, the X100VI's on a church's plain corner; 37 files with none read. 91 library tests pass, a sharp edge in focus, the same blurred not in focus, a plain part and a lone straight edge can't tell among them.
- [ ] A point the camera truly missed says "not in focus here" on a real photo: no sample has one; the owner's photos.
- [ ] A Sony body that always records its middle is told by the owner's own photos.

## Sources

- PhotoMatch's owner, on 2026-11-26.
- ExifTool's Sony, FujiFilm and Olympus tag pages; the Focus Points plugin's Fujifilm and Sony readers (Apache 2.0); digiKam's readers, for their meaning only (GPL); read on 2026-11-26.
- `examples/focus_points.rs` over PhotoMatch's samples, on the owner's Mac.
