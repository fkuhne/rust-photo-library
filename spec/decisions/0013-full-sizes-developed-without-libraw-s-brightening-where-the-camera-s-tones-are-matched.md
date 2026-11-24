# 0013. Full sizes developed without LibRaw's brightening, where the camera's tones are matched

- Status: Accepted
- Date: 2026-11-23

## Context

PhotoMatch's plan step 2.59. On the owner's PC, on 2026-11-22, zooming into a portrait from the owner's Sony α7R III showed flat pink patches on the skin's highlights, which the camera's preview, shown before the zoom, didn't have. A photo without a full-size preview is developed from its RAW data at full size by LibRaw, then its tones are matched to the camera's preview, channel by channel (`raw::match_tones`), so that zooming in shows more detail, not another look.

What was found on 2026-11-22 and 23, with the owner's photo and the samples, on the owner's Mac:
- **LibRaw brightens a photo by its histogram:** `auto_bright_thr`, the "portion of clipped pixels when auto brightness increase is used", is "0.01 (1%) for dcraw compatibility"; `no_auto_bright` turns it off; for modern low-noise cameras its documentation finds 0.001 to 0.00003 reasonable ([LibRaw's data structures](https://www.libraw.org/docs/API-datastruct.html), read on 2026-11-22).
- **That brightening is what flattens the skin:** on the owner's photo, 1.12% of LibRaw's pixels had red at the top (250 or more) while green wasn't (under 235), and 0.83% once its tones were matched; the camera's preview had 0.00%. With `no_auto_bright`, 0.00%, and 0.01% matched. Highlight recovery (`highlight` 2, blend) changed nothing, as no channel was clipped by the sensor.
- **On the samples, at full size,** with the tones matched: no brightening brought each photo's flat red down to its camera preview's, or below, where the brightening had raised it. The Canon EOS 5D Mark IV went from 0.60% to 0.01%, the Leica Q2 from 0.55% to 0.02%, the Olympus E-M5 Mark II's `.ORI` from 1.03% to 0.00%, and the Leica M10-R from 0.33% to 0.00%. None rose above its preview by more than 0.06 points. A threshold of 0.1% left 0.09% to 0.12% on those three.
- **The tone matching sets the brightness anyway:** the owner's photo, developed both ways, has the same mean levels, R 107.2 against 107.3, G 102.6 and 102.8, B 92.1 and 92.3.

The owner chose on 2026-11-23: "A", no brightening by LibRaw where the tones are then matched.

## Decision

**Where a photo developed from its RAW data is matched to the camera's preview, LibRaw doesn't brighten it** (`no_auto_bright`, in `raw::develop`). That's every full size developed with a preview to match. A photo developed without one keeps LibRaw's brightening, as its tones come from LibRaw alone.

**The full sizes kept before are made again:** their names in the cache carry `FULL_VERSION`, now 2, so a full size kept by the older development isn't served. Those older files end in `-full.jpg`, as the newer ones do, and the cache's 2 GB limit removes them, the oldest first. Thumbnails and screen-sized images are made as before, and kept.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| A threshold of 0.1%, `auto_bright_thr` 0.001 | Left flat red on three samples, 0.09% to 0.12% |
| Highlight recovery, `highlight` 2 | Changed nothing here: the sensor clipped nothing |
| Matching the tones by brightness alone, keeping each pixel's hue | A larger change, for patches the brightening alone made |
| `CACHE_VERSION` raised | Would make every thumbnail again, and leave the old ones on disk |

## Consequences

- **Every full size is developed again once,** at the next zoom, in an app that takes this version.
- **The screen-sized image developed for an older camera's small preview** isn't matched to anything, so it keeps LibRaw's brightening.

## Assumptions to verify

- [x] The owner's photo, through `raw::read_developed` at full size, as the app makes it: red at the top while green isn't on 0.00% of its pixels, against 0.65% before, its mean levels the same. On 2026-11-23, PhotoMatch's `develop_once … full` with this library.
- [x] A full size kept under the older name is developed again: `a_full_size_kept_by_an_older_development_is_made_again`; 47 tests pass, 51 with `places`.
- [ ] The owner sees no pink at full size on the PC: PhotoMatch's next Windows build.

## Sources

- The owner, on 2026-11-22: the screenshots of the patches; on 2026-11-23: "A".
- [LibRaw's data structures](https://www.libraw.org/docs/API-datastruct.html): `auto_bright_thr`, `no_auto_bright`, `highlight`.
- PhotoMatch's trial, `examples/pink_probe.rs`, not kept: the owner's photo, and the samples' RAW files at full size, under each setting.
