# 0019. Focus marks only where sharp detail crosses: a blur of motion left out

- Status: Accepted
- Date: 2026-11-26
- Supersedes, in part: [0018](0018-focus-peaking-marks-from-the-raw-data-fine-detail-over-the-photo-s-own-noise.md), its marks of every sharp step, and its smallest detail in an image file

## Context

PhotoMatch's owner, on 2026-11-26, after trying record 0018's marks on a photo of a train going by, a 1,280-pixel JPEG of 212 KB: the train and the ceiling covered in marks, nothing of it in focus but the platform; then, "remember that we need to detect focus, not sharpness. i am not sure to how extent we can treat both equal". Of the evaluation and the fixes laid out that day: "go with your recommendations".

What was found on 2026-11-26:
- **A blur of motion leaves crisp edges one way:** the train's streaks were sharp down, smooth across, and record 0018 took a step sharp either way. Out-of-focus blur spreads every way; a blur of motion or of the camera's shake runs one way. Research on telling blur types apart builds on such differences: Liu, Li and Jia, "Image Partial Blur Detection and Classification", CVPR 2008, as a search quoted it; the paper itself unread.
- **Locally, a straight edge in focus looks like a streak;** around it, a subject in focus has detail crossing it, the moving parts of a photo none.
- **A small, compressed image file shows no noise:** compression flattened the JPEG's quiet parts, its noise came out near nothing, and its own edges passed record 0018's smallest detail, a 250th of its range.
- **Tried on the photos,** each version's marks drawn over them and looked at: crossing in blocks of 16 or 32 steps alone left the stone frog's outline, sharp but curving gently one way, unmarked; judged with the neighbouring blocks, its nose kept its marks.

| Photo | Marked at Mid, record 0018 | With this record |
|---|---|---|
| The train, a JPEG | 14.0%, the streaks and the ceiling | 3.8%, the platform, still and in focus |
| ZV-E10, the cactus | 0.89% | 0.77% |
| α6400, the stone frog | 0.23% | 0.14%, the outline of its head lost |
| R5 Mark II, the label | 0.48% | 0.45% |
| α9 III, the pegboard | 11.43% | 10.97% |

## Decision

**A sharp step stands only where, in its block of 32 steps and the eight around it, sharp steps run both across and down, the fewer at least a fifth of the more; an image file's smallest detail is 3 of its 255 levels.**

- **The blocks:** 32 by 32 steps, about 64 photosites of a Bayer sensor, counted with their eight neighbours, about 96 steps a side, so that a subject's own crossing detail nearby keeps an edge of it.
- **An image file:** its smallest step 3 levels, as its compression can hide its noise; a RAW file's stays a 250th of its range.
- **What it costs:** a straight edge in focus with no crossing detail within about 48 steps, as the outline of a smooth object against a soft background, isn't marked.
- **The kept marks** move to version 2, so that none made before is shown.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Every sharp step, as record 0018 | A blur of motion is marked as if in focus |
| Crossing in each block of 16 or 32 alone | More of a subject's gentle outlines lost |
| A model of blur types, as research does | Training data and a model, for the Intelligent Features |

## Consequences

- **The marks mean "in focus and still"** more than "sharp": PhotoMatch's key says so.
- **A test of a texture blurred one way** fails without the rule, marking 12.7% against 40% still, and passes with it.
- **A small image file** still shows little of focus, its pixels too few: PhotoMatch says so.

## Assumptions to verify

- [x] The train's streaks aren't marked, and the samples' subjects still are: the table above, looked at over the photos.
- [ ] The owner's photos with motion, and with gentle outlines in focus, look right: the owner's look.

## Sources

- PhotoMatch's owner, on 2026-11-26.
- Liu, Li and Jia, CVPR 2008, as a search quoted it.
- `examples/focus.rs` over the owner's photo and PhotoMatch's samples, on the owner's Mac, 2026-11-26.
