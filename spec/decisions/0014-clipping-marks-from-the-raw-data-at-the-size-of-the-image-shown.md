# 0014. Clipping marks from the RAW data, at the size of the image shown

- Status: Accepted
- Date: 2026-11-24

## Context

PhotoMatch's plan step 2.68, and its record 0051: the photo view marks where a photo's highlights clip and its shadows sink. The owner, on 2026-11-24: "it must read the raw data not the embedded image", and the computation must not block the app's main thread. The library reads RAW files, so the marks are made here, and each app shows them.

What was found on 2026-11-24, on the owner's Mac (Apple M3, 8 GB, macOS 14.3), with a probe over PhotoMatch's 51 RAW samples:
- **LibRaw gives each photosite's value, the black level and the white level** (`rawdata.raw_image`, or a colour per pixel in `color3_image` or `color4_image`; `color.black` and `cblack`; `color.maximum`, "calculated from the data for most cameras, hardcoded for others"; [LibRaw's data structures](https://www.libraw.org/docs/API-datastruct-eng.html), read on 2026-11-24).
- **Reading the data takes 5 to 965 ms per file,** the X-Trans files the slowest, and checking every photosite 5 to 112 ms. One sample, the Nikon Z 8's, LibRaw 0.22.3 can't unpack ("FileUnsupported").
- **Clipped photosites sit at or above the white level:** up to 5.7% of a channel on the samples, some Sonys' above it, as 16,628 over 16,383. At the black level itself, 0.00 to 0.33% of photosites, as noise keeps them over it; 8 stops under the white level or more, 0 to 15%.
- **The camera's preview and LibRaw's visible area aren't always the same frame:**
  - the ZV-E10's 16:9 preview covers part of its 3:2 sensor;
  - LibRaw's crop of the camera's own framing, `raw_inset_crops[0]`, has the preview's shape on most cameras, but not on the GH7, whose crop is 3:2 and preview 4:3;
  - many files have no crop;
  - the crop is counted from the edge of the whole sensor, margins included: the 5D Mark IV's at 148, 54 within its margins of 136, 42.
- **FastRawViewer**, by LibRaw's authors, marks overexposure as clipping, and takes 8 stops under it as underexposed by default: "a standard value for the photographically usable dynamic range" (its manual, as a search quoted it). **darktable** marks raw clipping at the white level by default ([its manual](https://docs.darktable.org/usermanual/4.2/en/module-reference/utility-modules/darkroom/raw-overexposed/)).

## Decision

**`clipping::of_raw` marks a RAW file from its sensor data, and `clipping::of_image` an image file from its pixels, at the size of the image they go over, upright. `ThumbCache::clipping` makes them once, as a PNG, and keeps them.**

- **A highlight:** a pixel any of whose photosites is at or above LibRaw's white level.
- **A shadow:** a pixel whose photosites are, on average, 8 stops or more under the white level, counted from their black levels: the sum of their values over black, times 256, at most the sum of their ranges.
- **In an image file:** a highlight where a pixel has a channel at 255; a shadow where at least half of the pixels are black.
- **The part of the sensor shown,** within 1% of the image's shape, the closer one winning:
  - LibRaw's crop, when the image has its shape;
  - or else the visible area;
  - or else the visible area cut to the image's shape, centred.
- **Turned as the photo is,** by LibRaw's `flip`, as the photo's own images are.
- **The PNG:** red, 255, 0, 0, over highlights, and blue, 0, 96, 255, over shadows, as Lightroom Classic shows clipping; clear elsewhere.
- **Kept** under `<key>-clip-v1-<width>x<height>.png`, beside the large view's images, so that another size or a new version makes them again.
- **A file LibRaw can't unpack** gets `DevelopFailed`, saying so.
- **Nothing here runs on an app's main thread:** the app calls it from a worker, as for the large view's images.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| The camera's preview, as Lightroom's Library would | The owner: the RAW data, not the embedded image |
| Shadows at the black level | 0 to 0.33% of photosites on the samples: almost never shown |
| A colour for each channel clipped, as FastRawViewer | The owner chose Lightroom's two colours (PhotoMatch's record 0051) |
| The marks at the sensor's full size | Up to 61 MP to send and decode for each photo, where the screen shows a few |
| The visible area always | The ZV-E10's 16:9 photo would get 3:2 marks, out of line |

## Consequences

- **`raw::open` and `raw::orientation` are the crate's,** shared with this module.
- **An app passes the size of the image it shows,** and stretches the marks over it when zoomed in, a mark then covering a few of the photo's pixels.
- **A camera whose sensor saturates under LibRaw's white level** would show no marks where it clips; none of the samples did.

## Assumptions to verify

- [x] The marks line up with the photo shown. On 2026-11-24, `examples/clipping.rs` over the samples: the ZV-E10's 16:9 marks on its sunglasses and lettering, the X100VI's turned upright, the Z 6II's on the sky between branches.
- [ ] They line up on the owner's own photos, from the α7R III: PhotoMatch's check on the PC.

## Sources

- PhotoMatch's owner, on 2026-11-24; PhotoMatch's record 0051.
- LibRaw's data structures; FastRawViewer's manual, as a search quoted it; darktable 4.2's manual; read on 2026-11-24.
- The probe and `examples/clipping.rs` on PhotoMatch's samples, on the owner's Mac.
