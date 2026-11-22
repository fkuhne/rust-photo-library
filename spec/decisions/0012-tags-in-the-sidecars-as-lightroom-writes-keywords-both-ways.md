# 0012. Tags in the sidecars, as Lightroom writes keywords, both ways

- Status: Accepted
- Date: 2026-11-22

## Context

The store keeps each photo's tags beside its rating, but no sidecar held them ([0011](0011-the-photographer-s-tags-in-the-store-beside-the-rating.md)). PhotoMatch's plan step 2.56 brings them there, so that Lightroom Classic and Capture One see them as keywords, hierarchical ones included. A RAW file's rating already goes both ways through its XMP sidecar, the newer change winning, a key pressed on top of another app's change, everything else in the sidecar kept ([0008](0008-ratings-the-model-the-store-and-the-sidecars-move-whole.md)).

What was read, on 2026-11-21 and 2026-11-22:
- **exiftool's XMP tags and its source, `XMP.pm`:** `dc:subject`, Dublin Core's, `http://purl.org/dc/elements/1.1/`, and `lr:hierarchicalSubject`, Lightroom's, `http://ns.adobe.com/lightroom/1.0/`, each a list of text.
- **Users' reports in Adobe's community:** Lightroom Classic writes into a sidecar both, `lr:hierarchicalSubject` with each keyword's path between `|`, as `Travel|Portugal|Lisbon`, and `dc:subject` with every level as a keyword of its own; it saves a photo's whole set each time.
- **Capture One:** its pages refused to be read; users report hierarchies lost between it and other apps (unchecked).
- **`xmp_toolkit` 1.12.1:** `property_array` reads a list, `append_array_item` writes one item, an `XmpValue` marked an array making a Bag.

The owner chose for PhotoMatch on 2026-11-22: as Lightroom writes them; both ways.

## Decision

**Written as Lightroom Classic writes keywords** (`sidecar::write`, which now takes the tags): each tag in `lr:hierarchicalSubject`, its levels between `|`; and every level of every tag in `dc:subject`, once, sorted. Both lists are written whole from the photo's tags, all else in the sidecar kept.

**Read both ways, as a rating is** (`sidecar::read_with_tags`): each path in `lr:hierarchicalSubject` is a tag with its levels; each keyword in `dc:subject` that isn't a path or a level of one is a tag of its own, as Capture One or another app that writes only `dc:subject` leaves them. A sidecar another app changed since PhotoMatch last wrote or read it gives the photo its rating and its tags, the newer change winning; a key or a tag on top of that change reads it first.

**The store** keeps, beside the tags, those it last wrote to the sidecar or read from it, `synced_tags`; where they differ, a write is owed, as for a rating. A tag changed in an app counts as a change of the photo, its time the newer one; the photos of a change of tags have their sidecars written in the background, as a rating's. `Synced` says which photos' tags a sync took from their sidecars, `retagged`; `Changed`, a rating's change, gives the tags after it too, which a sidecar read on the way may have changed.

**Photos without a sidecar,** a DNG or an image file, keep their tags in the store alone, as their rating (PhotoMatch's record 0014).

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| The whole tag as one keyword in `dc:subject` | Lightroom would list "Travel > Portugal > Lisbon" as a keyword of its own, not its hierarchy |
| PhotoMatch writing, never reading back | It would have to merge its tags into the sidecar's, and couldn't tell a keyword removed in Lightroom from one never there |
| A time of change for tags apart from the rating's | A sidecar has one time; another app saves the whole photo at once |

## Consequences

- **Keywords a photographer gave in Lightroom become tags** the first time an app reads the sidecar; tags given in an app reach Lightroom when it reads the sidecar (Metadata → Read Metadata from Files).
- **Tags kept in a store before this,** step 0011's, are written to their sidecars at the next sync, as a write owed.
- **`sidecar::write` takes the tags:** an app writing a sidecar gives the photo's tags, or `&[]` to leave none.
- **Capture One is checked by hand** in PhotoMatch's step 5.3.

## Assumptions to verify

- [x] Keywords written so read back as the same tags; a sidecar as Lightroom writes it, and one with `dc:subject` alone, read as expected: the library's tests pass on 2026-11-22, `keywords_read_as_tags_and_are_written_as_lightroom_writes_them`, `tags_go_to_the_sidecar_and_another_app_s_newer_keywords_come_back` and `tags_kept_before_the_sidecars_held_them_join_their_keywords`; Lightroom's own fields, its keyword among them, still kept; 46 tests, 50 with `places`.
- [ ] Lightroom Classic shows the tags as its keywords, with their hierarchy: on the owner's PC, Lightroom Classic 8.2.1.
- [ ] Capture One shows them: PhotoMatch's step 5.3, with its trial.

## Sources

- The owner, on 2026-11-22: "let's follow your recommendations".
- exiftool's `TagNames/XMP.html` and `lib/Image/ExifTool/XMP.pm`, read on 2026-11-22; Adobe community threads on `dc:subject` and `lr:hierarchicalSubject`, read on 2026-11-21; `xmp_toolkit` 1.12.1's `src/xmp_meta.rs`.
