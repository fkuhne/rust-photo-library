# 0010. A photo's position, and its place named offline from GeoNames

- Status: Proposed
- Date: 2026-11-21

## Context

PhotoMatch's plan step 2.54 groups photos by place, from their GPS, as one more grouping in its Group By. PhotoBook may group by trip later. The library read no position, and the apps never go online (PhotoMatch's record 0018), so a place's name has to come from data inside the app.

Read and tried on 2026-11-21:
- **Positions in the files:** LibRaw parses a RAW file's GPS into `other.parsed_gps`, degrees, minutes and seconds with their references, and `gpsparsed`; the EXIF crate reads an image file's, HEIC included. Of the 67 sample files, only the iPhone JPEG and the HEIC made from it carry one; several RAW files have an empty GPS block, `gpsparsed` 1 and every value 0.
- **Positions in sidecars:** the XMP specification's `exif:GPSLatitude` and `exif:GPSLongitude`, as `DDD,MM,SSk` or `DDD,MM.mmk` (the Exif 2.32 XMP mapping, as exiv2's table of XMP tags and others quote it); `xmp_toolkit` 1.12.1 converts them, `xmp_gps::exif_latitude_to_decimal`. Lightroom Classic's Map module writes a position set by hand there (unchecked).
- **Place names:** GeoNames' lists of towns, under CC BY 4.0, which allows commercial use with credit (its readme; the licence's deed). Its towns of over 1,000 people, `cities1000`, are 172,046 places, 2.46 MB as five columns compressed with gzip; those over 15,000, 34,248, 0.52 MB. `admin1CodesASCII.txt` names the regions, `countryInfo.txt` the countries.
- **Naming by the nearest town errs at a city's edge:** the iPhone sample, taken in San Francisco near the Golden Gate, is nearest to Sausalito, across the water. The largest town within 15 km names Versailles' palace after a district of Paris. A circle for each town, 0.01 km for each square root of its people, at least 1 km, and the largest town whose circle holds the photo, named all seven spots tried as a person would: San Francisco, Paris for Montmartre, Versailles, Sausalito, New York City for the Brooklyn Bridge; Yosemite Valley, by the nearest town, 1.5 km off; and Jökulsárlón in Iceland, 52 km from any town, by its region.
- **`rrgeo`** (`reverse_geocoder` 4.1.1, MIT or Apache) looks places up offline, by the nearest, from a list it doesn't credit.
- **`flate2`** 1.1.10 is in the library's build already, through `png` and `tiff`.

The owner chose for PhotoMatch on 2026-11-21, "as you said": towns from `cities1000`, with the circles; town, region and country; the position from the file, else from its sidecar; the details read again without remaking thumbnails; the code in this library, the place list behind a feature; a sample folder of positions.

## Decision

**A photo's position,** `PhotoInfo::position`, in degrees, north and east positive: from a RAW file's GPS through LibRaw, from an image file's EXIF; none when the file has no GPS, or an empty block, which gives 0 and 0. And `sidecar::position(photo)`, from a RAW file's XMP sidecar, which an app reads when the file has none, fresh each time, as another app may set it.

**The details are read again, the thumbnails kept:** `PhotoInfo` carries the version of what it holds, `details_version`. Cached details from before a version are read again from the file, a read of its smallest preview, and the thumbnail and its size are kept. The cache's own version doesn't move.

**The place,** behind the Cargo feature `places`: `places::place_of(position)` gives a town, if one is near, its region and its country, in English, from GeoNames' `cities1000` of 2026-11-21, with its regions and countries:
- the largest town whose circle holds the position, 0.01 km for each square root of its people, at least 1 km; neighbourhoods (feature code `PPLX`) left out;
- else the nearest town within 10 km;
- else no town, and the region and the country of the nearest town within 100 km;
- else no place.

The list is a file the library builds in, `data/places.tsv.gz`, made by `scripts/places-data.py` from GeoNames' files, which it names with their date and SHA-256: six columns, its name, latitude, longitude, country, region code and people. It's read the first time a place is asked for, and looked up through a grid of half-degree cells.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| The nearest town, as `rrgeo` names it | San Francisco's edge named after Sausalito; and its list isn't credited, which CC BY asks |
| GeoNames' towns over 15,000 | 0.4 MB instead of 2, but Yosemite Valley named after a town 91 km away |
| Regions and countries only, from boundaries | No town, which is what a trip is remembered by |
| Remaking every thumbnail, by the cache's version | The owner's 2,564 photos took 150 s on the PC's hard disk; their details alone take a read of each file's smallest preview |
| The place list in every app | PhotoDev has no use for 2 MB of towns; a feature keeps it out |
| Downloading GeoNames at build time | A build that changes with GeoNames' daily files, and needs the network |

## Consequences

- **The library's git holds a 2.5 MB data file,** regenerated by its script; a new GeoNames snapshot is a change of its own, with its date.
- **An app with `places` credits GeoNames,** under CC BY 4.0, saying the list was cut to six columns; PhotoMatch in its third-party notices, which About shows.
- **About 6 MB of memory** once the list is read, and its reading's time, once (computed; measured in step 2.54).
- **Cached details are read again once,** after an app takes this version.

## Assumptions to verify

- [ ] The seven spots name as above, through the library: its tests.
- [ ] The list's memory and the time to read it, on the planning Mac.
- [ ] A sidecar's position, as Lightroom Classic writes one, reads (unchecked; the owner's Lightroom on the PC can make one).

## Sources

- The owner, on 2026-11-21: "as you said".
- GeoNames' `readme.txt` and its file listing, `download.geonames.org/export/dump/`; Creative Commons' CC BY 4.0 deed; all read on 2026-11-21.
- `xmp_toolkit` 1.12.1, `src/xmp_gps.rs`; LibRaw 0.22.3, `libraw_gps_info_t`; exiv2's table of XMP exif tags, `exiv2.org/tags-xmp-exif.html`.
- `rrgeo`'s README (GitHub) and `reverse_geocoder` 4.1.1 on docs.rs, read on 2026-11-21.
- Our runs: PhotoMatch's `docs/photo-browser.md`, step 2.54.
