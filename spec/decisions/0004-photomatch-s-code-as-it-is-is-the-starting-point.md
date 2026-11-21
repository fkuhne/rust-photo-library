# 0004. PhotoMatch's code, as it is, is the starting point

- Status: Accepted
- Date: 2026-11-20

## Context

PhotoMatch's library code is the original. PhotoDev's is a copy of seven of its files, unchanged but for module paths, the app's name, and one function made visible ([`docs/inventory.md`](../../docs/inventory.md)). Each app has its own patched `rsraw`. PhotoMatch is released, so its behaviour mustn't change. It's also being worked on: it moved from commit `9217996` to `b959990` on 2026-11-20.

## Decision

- **The library starts from PhotoMatch's files at the commit in hand when the move is made,** moved with their history in mind: the same code, with module paths and names changed, and nothing else at first. Its tests come with them.
- **PhotoDev's additions are merged in,** each marked: `rsraw`'s, and the one visible function.
- **`rsraw` is merged from the two copies:** PhotoMatch's additions and PhotoDev's, each marked by whose it was.
- **What moves to the library is removed from each app** in the same step that points the app at it. So no app keeps a copy that could drift.
- **A behaviour change** in the library, beyond what the move needs, is a step of its own, with a record.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Start from PhotoDev's copy | It's a copy of PhotoMatch's; PhotoMatch's is the original, and has moved on since |
| Rewrite it, cleaner, for the library | PhotoMatch's behaviour would change, untested, in a released app |

## Consequences

- **PhotoMatch's own tests,** moved with its code, check that nothing changed.
- **PhotoMatch's work in progress** must be committed, or left untouched, before its files move. Its `git status` is checked first.
- **Some names will read oddly** in a library, such as problems worded for PhotoMatch's interface. They change later, each in a step of its own.

## Assumptions to verify

- [x] PhotoMatch's tests pass on the library as they did on its own copy. Step 2.1, on 2026-11-21: the library's 34 tests, the ones that moved from PhotoMatch among them, and the 2 from outside pass here at `9e1b65d`; PhotoMatch's 3 that stayed pass there, and its self-check, 236, 236, 233, 185 and 218 checks ok in its five sample folders, none failed, as on its own copy that morning. Its thumbnails and details came out byte for byte the same, and its sidecars the same but for the time they were written (PhotoMatch's `docs/photo-browser.md`, step 2.57).

## Sources

- The owner, in PhotoDev's conversation of 2026-11-20: "you can change PhotoMatch's repo as you see fit".
- `../photomatch` at its commit `b959990`, and `../photodev/app/core/src/library/`.
