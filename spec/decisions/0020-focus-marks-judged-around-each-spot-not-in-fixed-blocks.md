# 0020. Focus marks judged around each spot, not in fixed blocks

- Status: Accepted
- Date: 2026-11-26
- Supersedes, in part: [0019](0019-focus-marks-only-where-sharp-detail-crosses-a-blur-of-motion-left-out.md), its blocks of 32 steps judged with the eight around them

## Context

PhotoMatch's owner, on 2026-11-26, of focus peaking over a photo of a cyclist in a street, a 1,280-pixel JPEG: "I am ok with the focus peaking for now, although it is still not perfect. I noticed that in some images it appeared like patches." Of the cause and the fix laid out that day: "yes, commit the fix".

What was found on 2026-11-26: record 0019 judged the photo in fixed blocks of 32 by 32 steps, each with the eight around it, and kept or dropped a block's marks whole; where a kept block met a dropped one, the marks stopped along its straight edge, at the left of the cyclist's head and the right of his bicycle. Tried on that photo, with each step judged by the window around its own small cell: the straight edges gone, the marks thinning out as the photo does; the marked share the same, 1.67% at Mid; the train, 3.83% against 3.82%, and the samples' subjects, as before.

## Decision

**Each cell of 4 by 4 steps is judged by the window of 25 by 25 cells around it, about 96 steps a side, as record 0019 judged its blocks: sharp steps both across and down, the fewer at least a fifth of the more.**

- **The window's sums** come from running sums over the cells, so each cell's costs four additions.
- **The kept marks** move to version 3.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Record 0019's blocks of 32 | Marks cut off along straight edges |
| A window around every step, at the step's own resolution | Two sums the size of the photo, about 120 MB for a 61 MP sensor (computed); cells of 4 steps are as fine on screen |

## Consequences

- **The marks follow the photo,** not a grid; the crisp streaks of motion beside a subject still mark where detail crosses them nearby, a limit only a model of blur types would lift.

## Assumptions to verify

- [x] The straight edges are gone and the marks otherwise as before: the cyclist, the train and the samples drawn and looked at, 2026-11-26; the library's 88 tests pass, 92 with `places`.

## Sources

- PhotoMatch's owner, on 2026-11-26.
- `examples/focus.rs` over the owner's photos and PhotoMatch's samples, on the owner's Mac.
