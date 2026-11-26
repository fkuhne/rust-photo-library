# 0018. Focus peaking marks from the RAW data: fine detail over the photo's own noise

- Status: Accepted
- Date: 2026-11-26

## Context

PhotoMatch's plan step 2.75, and its record 0062: the photo view and Compare mark where a photo is sharp, as cameras' focus peaking and Capture One's Focus Mask do, from a RAW file's own sensor data as the clipping marks are (record [0014](0014-clipping-marks-from-the-raw-data-at-the-size-of-the-image-shown.md)). The owner, on 2026-11-26, of the choices laid out: "go with your recommendations".

What was known on 2026-11-26:
- **darktable** ([`src/common/focus_peaking.h`](https://github.com/darktable-org/darktable/blob/master/src/common/focus_peaking.h), at `08010e2adc`, read; GPL, so its method only, none of its code): on the image as displayed, a luminance, a noise filter, then the gradient close up minus 0.67 of the gradient a little further, "if both gradients have the same magnitude, it means we have no sharpness but just a big step in intensity"; marks where that passes the photo's own mean plus 2.5, 5 and 10 times its spread, in blue, green and yellow.
- **FastRawViewer** ([Outlining In-Focus and Highly Detailed Areas](https://www.fastrawviewer.com/node/233), read): "High contrast edges" and "Fine details", "small, acute, but not necessarily contrasting details", in green and red; P cycles them.
- **Capture One's Focus Mask** is "influenced to a degree by both image resolution and noise levels", with a threshold its users set by camera and ISO (Capture One's support, as a search quoted it).
- **Sony's cameras:** Peaking Level High, Mid or Low; Peaking Color Red, Yellow, Blue or White; "Since the product recognizes sharp areas as in focus, the effect of peaking differs, depending on the subject and lens" ([α7R V help guide](https://helpguide.sony.net/ilc/2230/v1/en/contents/TP0002925721.html), read).
- **Noise from the data itself:** scikit-image's `estimate_sigma` takes "the median absolute deviation of the wavelet detail coefficients", after Donoho and Johnstone, Biometrika 81.3 (1994), §4.2, assuming Gaussian noise (scikit-image 0.26.0's reference, read). The median of a normal value's size is 0.67449 of its spread (computed, Python's `statistics.NormalDist`).
- **The sensor data:** as record 0014 found, LibRaw unpacks it in 5 to 965 ms; a Bayer sensor repeats its colours every 2 by 2 photosites, an X-Trans one every 6 by 6, each 3 by 3 of it holding 5 green, 2 red and 2 blue (LibRaw's `idata.filters`, 9 for X-Trans).

## Decision

**`focus::of_raw` marks where a RAW file is sharp, from its sensor data, and `focus::of_image` an image file from its pixels, at the size of the image they go over, upright, at one of three levels; `ThumbCache::focus` makes them once, as a PNG in the colour asked, and keeps them.**

- **A brightness at each full colour of the sensor:** the photosites of each 2 by 2 block of a Bayer sensor, or each 3 by 3 of an X-Trans one, counted from its corner of the pattern, added up over their black levels; a pixel's channels in a linear DNG or an image file. Its square root, so that the sensor's noise, which grows with the light, is about the same everywhere.
- **Fine detail:** across and down, the slope over one step either side against the slope over three, as darktable's idea has it in its own way: a sharp edge is steeper close up than further out, a soft one isn't. A step is sharp, across or down, where both hold:
  - **its shape:** the close slope at least 1.6, 1.8 or 2.2 times the far one, at High, Mid and Low; a sharp step's is 3, blurred by 1 step 2.05, by 1.5 steps 1.56, by 2.5 steps 1.21, whatever its contrast (computed, for a Gaussian blur), so that a soft edge of high contrast doesn't count in a photo with next to no noise;
  - **over the noise:** the close slope above the far one by 3, 4.5 or 7 times the noise of the brightness there; noise alone, its close slope spread 0.71 times the noise, passes 3 times it once in 45,000 steps (computed).
- **An edge continues:** a step marks only where a step in the row beside it along the edge, beside it or one over, marks too, so that a hot photosite or a grain of noise doesn't.
- **The photo's own noise, by brightness:** the median size of the step from each brightness to the next, over 0.67449 and √2, in blocks of 16 by 16 steps; in each of 24 bands of brightness, the quietest quarter of its blocks, where only the sensor's noise shows, the gaps and smooth patches every brightness has somewhere; a band with fewer than 6 blocks takes the nearest band's. The square root evens out the light's own noise, but not the sensor's read noise, which it makes larger in the shadows: one noise for the whole photo marked the noise of a dark board; one from all a band's steps took foliage for noise.
- **At the size of the image shown:** each mark pixel takes the sharpest step of the sensor under it, so that a fine sharp detail isn't averaged away; where the image has more pixels than the sensor has colours, as a 30 MP sensor's 3,360 under a screen's 3,420, each the step it lies on; over the same part of the sensor as the clipping marks, `clipping::area_for`.
- **The marks** in one colour, clear elsewhere.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Compared with the photo itself, as darktable's | A photo out of focus still shows its least soft parts as sharp |
| A fixed level alone, as Capture One's | Its users set it again for each camera and ISO, its noise taken for sharpness |
| Two filters, as FastRawViewer's | Two things to read where one answers whether it's sharp |
| The image shown, as darktable's | A 24 MP photo shown at 3 MP hides a blur of a pixel or two |
| The camera's preview | Its sharpening makes a soft photo look crisp; older Sonys' are 1,616 pixels across |
| A demosaicked image | The demosaic's own detail where the sensor has none, and a second's work |

## Consequences

- **Each photo's marks take its data's unpacking, 5 to 965 ms, and a pass or two over it,** with a brightness for each block kept while they're made: about 60 MB for a 61 MP sensor (computed).
- **A photo with nothing sharp shows few marks or none;** one sharp throughout shows its texture marked.
- **The levels' factors were set on synthetic squares,** sharp, blurred and noisy, and the marks looked at over the samples: on the ZV-E10's knitted cactus and not its soft background; the α6400's stone frog and not its garden; the R5 Mark II's label text and the cloth at its distance, not its soft tomatoes; a sharp landscape's lit foliage, not its near-black trees, whose detail is no larger than the sensor's noise there.
- **No sample is above ISO 1,000;** a high-ISO photo is the test's and the owner's look.

## Assumptions to verify

- [x] The marks fall on the sharp parts of the samples, and few on the soft ones: on 2026-11-26, `examples/focus.rs` over the RAW samples of the Sony, other makers', older formats' and places' folders and the image files, looked at over the photos (above).
- [x] A synthetic sharp edge marks, the same edge blurred and plain noise don't, nor a hot photosite, nor the shadows' own noise, nor a soft edge of high contrast in a clean photo: 12 tests.
- [x] The time a photo takes, over the samples: RAW files 57 ms to 1.1 s at a large view 2,560 across, the X-Trans ones and the α7 IV's the slowest, mostly LibRaw's unpacking; image files 6 to 50 ms. The Nikon Z 8's LibRaw can't unpack.
- [ ] A high-ISO photo's noise isn't marked: the owner's look.

## Sources

- PhotoMatch's owner, on 2026-11-26.
- darktable's `focus_peaking.h`, FastRawViewer's page, Sony's help guide and scikit-image's `estimate_sigma`, read on 2026-11-26; Capture One's Focus Mask, as a search quoted it.
- Record 0014 and `clipping.rs`.
