# 0009. Every still photo's RAW extension LibRaw reads and has the colours for

- Status: Accepted
- Date: 2026-11-21

## Context

The library lists a file as a RAW photo by its extension, `raw::is_raw`, and knew ten: `arw`, `cr2`, `cr3`, `nef`, `raf`, `dng`, `rw2`, `rwl`, `orf` and `pef`, PhotoMatch's. The owner's library holds 72 `.sr2` files the apps didn't list (PhotoMatch's plan step 2.52). On 2026-11-20, the owner chose for PhotoMatch, "as you said": every still photo's RAW extension LibRaw reads, `.raw` included, video RAW left out, each format tried on a CC0 sample from raw.pixls.us where there's one. The list lives here, so it's every app's.

What was read and tried, on 2026-11-21, on the owner's Mac (Apple M3), with LibRaw 0.22.3 as this library builds it:
- **LibRaw knows a file by its contents, not its name:** `src/metadata/identify.cpp` never reads the file's name. A file with a listed extension that LibRaw can't read shows as one the app can't read.
- **The extensions in use,** from digiKam's list of LibRaw's (`core/libs/rawengine/drawfiles.cpp`, KDE's GitLab, version 8 of its list, which takes its first versions from dcraw's), and raw.pixls.us's 2,012 samples (its listing, `json/getrepository.php`; 1,869 of them CC0).
- **LibRaw's camera list,** 1,258 cameras (`libraw_cameraCount`), and its colour table, `src/tables/colordata.cpp`: without a camera's colour matrix, the library doesn't develop its RAW data, as its colours would come out wrong (`knows_colours`).
- **LibRaw's changelog,** 0.20: "GoPro (via GPR SDK)"; "Foveon X3F support changed: it is supported only if USE_X3FTOOLS defined at build", as code imported from other projects is off by default. This library defines neither.
- **One sample of each new extension,** the smallest CC0 one on raw.pixls.us, its SHA-256 checked, read and thumbnailed by PhotoMatch's `thumbs_bench` and developed by its `develop_once`, through a local `[patch]`:

| Extension | Sample | Preview | Developed at half size |
|---|---|---|---|
| `3fr` | Hasselblad H3D | 3624 × 2722 | 726 ms |
| `arq` | Sony ILCE-7RM3, pixel shift | 1616 × 1080 | 7968 × 5320 in 1,109 ms, 1,087 MB at its peak |
| `crw` | Canon PowerShot G1 | 640 × 480 | 58 ms |
| `dcr` | Kodak DCS760C | 1520 × 1008 | 62 ms |
| `erf` | Epson R-D1 | 640 × 424 | 55 ms |
| `fff` | Hasselblad H5D-40 | 3664 × 2751 | 791 ms |
| `iiq` | Phase One P40+ | 1829 × 1370 | 252 ms |
| `kdc` | Kodak P712 | 3072 × 2304 | 105 ms |
| `mos` | Leaf Aptus 22 | 332 × 244 | 360 ms |
| `mrw` | Minolta DiMAGE 5 | 640 × 480 | 25 ms |
| `nrw` | Nikon COOLPIX P1000 | 4608 × 3456 | 118 ms |
| `ori` | Olympus E-M5 Mark II | 3200 × 2400 | 338 ms |
| `raw` | Panasonic DMC-FZ8 | 1548 × 1160 | 74 ms |
| `sr2` | Sony DSC-R1 | 640 × 424 | 63 ms |
| `srf` | Sony DSC-F828 | 640 × 480 | 87 ms |
| `srw` | Samsung NX500 | 6480 × 4320 | 438 ms |
| `sti` | Sinar eVolution 75 | 2496 × 3334 | 270 ms |
| `gpr` | GoPro HERO7 Black | none | not read: no colour matrix, nor GoPro's SDK |
| `mdc` | Minolta RD175 | none | not developed: no colour matrix |
| `x3f` | Sigma DP1 | | not opened: `FileUnsupported` |

Two things the older files showed:
- **Some embed a small preview,** 640 × 480, or the Leaf's 332 × 244, where the newer cameras tried embed 1616 × 1080 at the least (older Sony bodies'). The large view showed it as it was, 244 points wide in PhotoMatch's window, until zoomed in.
- **A Leaf back records no time,** and LibRaw gives 0, which the library turned into 1 January 1970: PhotoMatch showed the photo as taken on 31 December 1969, and its filter by date counted it.

## Decision

The library lists 32 extensions: PhotoMatch's ten; the 17 read and developed from a sample; and 5 more without a sample, whose cameras LibRaw names and has the colours of: `cap` (Phase One), `cs1` and `ia` (Sinar), `mef` (Mamiya ZD) and `qtk` (Apple QuickTake).

Left out:
- **Video RAW:** `.ari`, `.cine`, `.cin` and `.r3d`, as the owner chose.
- **`.hdr`,** which Radiance's HDR images share, so that those don't show as photos that can't be read.
- **What the apps can't show:** `.gpr` and `.x3f`, which need code this build leaves out; and the oldest cameras' formats, for which LibRaw has no colour matrix: `.mdc`, `.dc2` and `.k25` (Kodak DC20 and DC25), `.kc2` (Kodak DCS200), `.pxn` (Logitech Fotoman) and `.bay` (Casio QV).

And two changes to reading, found with the older files:
- **A preview under half the size asked for, and under 1280 pixels long, gives way to the RAW data developed at half size,** as no preview at all does: the large view of a 640 × 480 preview, and the Leaf's thumbnail. In keeping with PhotoMatch's record 0015, viewing quality before speed. A newer camera's preview never does, even on a 4K screen.
- **A time of 0 is no time:** the photo has no date, as a JPEG without one has.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Only the extensions with a sample, 27 | The five without one are formats of cameras LibRaw names and has the colours of; leaving them out would hide a photographer's files for want of a sample |
| Every extension LibRaw's cameras use, 40 or so | The ones it can't develop would show only as photos that can't be read |
| Turning on X3F, or adding GoPro's SDK | LibRaw leaves X3F off as code it doesn't vouch for against unexpected files; GoPro's SDK is more code to build for one camera maker's files |
| Small previews as they were | A photo of a supported camera shown as a stamp in the middle of the large view |
| A small preview judged against the size asked alone | On a 4K screen, the owner's α7R III's 1616 × 1080 preview would give way to developing, about a second a photo: another step's choice |

## Consequences

- **Each app lists the new formats** once it takes this commit.
- **Sidecars:** a photo in a new format gets its XMP sidecar, as every RAW file but a DNG does (PhotoMatch's record 0014).
- **The large view of an older camera's photo is developed,** which takes up to about a second (the table), where its preview showed at once.
- **The ARQ,** a pixel-shift file, takes about 1.1 GB to develop, on the planning Mac.
- **PhotoMatch's third-party notices name the X3F library in LibRaw,** which this build compiles to nothing, as `USE_X3FTOOLS` isn't defined: harmless, to correct with the notices' next change.

## Assumptions to verify

- [ ] PhotoMatch's self-check passes on the new samples, and its old folders' thumbnails stay byte for byte the same.
- [ ] `cap`, `cs1`, `ia`, `mef` and `qtk` files open: no sample to try (unchecked).
- [ ] A Leaf back's photo has no time to read, rather than one LibRaw misses (unchecked).

## Sources

- The owner, on 2026-11-20, for PhotoMatch's step 2.52: "as you said".
- LibRaw 0.22.3: `src/metadata/identify.cpp`, `src/tables/colordata.cpp` (`adobe_coeff`, `simple_coeff`), `src/x3f/*.cpp` (`#ifdef USE_X3FTOOLS`), `Changelog.txt`, the 0.20 entries.
- digiKam's `core/libs/rawengine/drawfiles.cpp` (invent.kde.org, `master`, read on 2026-11-21): the extensions, not its code.
- raw.pixls.us, its listing and its samples, read and downloaded on 2026-11-21; each sample's SHA-256 as listed.
- Our runs: PhotoMatch's `docs/photo-browser.md`, step 2.52.
