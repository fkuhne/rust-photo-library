# 0011. The photographer's tags, in the store beside the rating

- Status: Accepted
- Date: 2026-11-21

## Context

PhotoMatch's plan step 2.55: the photographer's own tags, added, renamed and removed on one photo or many, and filtered by; its step 2.56 then writes them into the sidecars as keywords, hierarchical ones included, for Lightroom Classic and Capture One. Each app keeps its picks, stars and labels in its own store, `Ratings`, a file of entries by path, and syncs them with the sidecars ([0008](0008-ratings-the-model-the-store-and-the-sidecars-move-whole.md)).

What Lightroom Classic does, read on 2026-11-21: keywords are typed separated by commas, `|`, `<` or `>` marking a hierarchy (its help, "Use keywords", through a search's excerpt, as the page refuses to be fetched); in a sidecar it writes `dc:subject`, each level as a keyword, and `lr:hierarchicalSubject`, the path as `Travel|Portugal|Lisbon` (users' reports in Adobe's community). Capture One's own pages refused to be read; users report hierarchies lost between it and other apps (unchecked).

The owner chose for PhotoMatch on 2026-11-21, "as you said": a tag is a name with levels, as Lightroom's; kept in the store beside the rating; renamed and removed on the open folder's photos.

## Decision

**A tag is its levels joined by " > ",** as "Travel > Portugal > Lisbon": typed with `>` or `|` between levels, commas between tags, each level trimmed, empty ones dropped (`ratings::tags_from`). A photo's tags are kept sorted, without repeats, letters' case kept as typed, a tag equal to another but for case counted as the same.

**The store keeps them beside the rating,** in each photo's entry, `tags`, left out of the file when there are none, so a store from before reads as it was, without tags, and a photo with tags but no rating is kept.

**Changing them:** `Ratings::retag(photos, change)` adds tags, removes them, or renames one, and returns each photo's tags before and after, for undo; `Ratings::set_tags(photos, tags)` puts them back. Removing or renaming a tag takes its levels below with it, as "Travel" takes "Travel > Portugal". Each saves the store, and puts it back as it was if the save fails.

**Not yet in the sidecars:** a change of tags writes no sidecar, and syncing leaves them as they are; step 2.56 of PhotoMatch's plan brings them there, with their own time of change, so that the newer change wins as a rating's does.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Flat names only | Step 2.56 hands hierarchies to Lightroom; flat names would mean reworking them |
| A file of their own | Two files to keep in step, and the sidecar's newer-wins rule split across them |
| Tags in `Rating` itself | `Rating` is what the sidecar sync compares and writes today; tags in it would write sidecars before step 2.56 decides how |
| Only in the sidecars | An image file has none; and a tag set in PhotoMatch must survive another app rewriting the sidecar |

## Consequences

- **The store's file grows by the tags' text.**
- **`Ratings::get` still gives the rating alone;** `Ratings::tags` gives the tags.
- **PhotoDev may read its own store's tags, or ignore them:** its file has none until it adds some.

## Assumptions to verify

- [x] A store from before reads, and keeps its ratings: the library's tests, `a_store_from_before_reads_and_keeps_no_tags_key`, pass on 2026-11-21; a store without tags is written without the key.
- [x] Tags survive a failed save, put back as they were: `tags_that_cant_be_saved_change_nothing` passes. And they stay when another app changes the sidecar, through a sync and through a key pressed on top of that change: `tags_stay_when_another_app_changes_the_sidecar`.

## Sources

- The owner, on 2026-11-21, for PhotoMatch's step 2.55: "as you said".
- Adobe's "Use keywords" for Lightroom Classic, through a search's excerpt; Adobe community threads on `dc:subject` and `lr:hierarchicalSubject`; read on 2026-11-21.
