# 0003. Private now, written so it can be opened

- Status: Proposed
- Date: 2026-11-20

## Context

On 2026-11-20 the owner said of the library: "we can even think in distributing it as an open source library or something that others can reuse it." The apps that use it are closed, so that they can be sold (Photomatch's record 0017, PhotoDev's 0005). Two options were offered the same day:
- private now, decided later, written so that it can be opened;
- open from the start.

The owner didn't pick, and went on to name the repository. The first is taken as a default, for the owner's review in step 0.2.

What the library holds, by licence:
- the owner's own code, from Photomatch and PhotoDev;
- `rsraw` and `rsraw-sys`, MIT;
- LibRaw, CDDL 1.0 or LGPL 2.1, used under CDDL 1.0, as Photomatch uses it (its record 0013).

## Decision

- **The repository is private for now.** Whether and when it's opened is decided later, in a record of its own.
- **Everything in it must allow publishing it under MIT or Apache 2.0,** and its use in a closed, paid app:
  - each dependency's licence is checked as it comes in, and recorded in `docs/inventory.md`;
  - nothing from GPL or AGPL code comes in;
  - no secret, path or name of the owner's private projects goes into the code, beyond what its documentation needs.
- **No licence file is chosen yet.** The code is the owner's, and the crate says `publish = false`.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Open from the start | It needs a licence chosen, documentation for others, examples and support, before the library has served its own apps. It can be done later at no loss, if it's written ready |
| Private, with no care to keep it openable | Opening it later would mean a licence audit and a clean-up |

## Consequences

- **Opening it later is a decision, not a rewrite.**
- **The apps' closedness is unchanged:** they depend on code the owner owns, and on MIT and CDDL code they already shipped.
- **Each step checks licences,** as the apps' steps do.

## Assumptions to verify

- [ ] Every dependency of the library allows MIT or Apache 2.0 publishing and closed use. Listed in `docs/inventory.md`, in step 1.1.

## Sources

- The owner, in PhotoDev's conversation of 2026-11-20.
- `../photomatch/spec/decisions/0013-build-libraw-0-22-3-from-a-pinned-copy.md`, and `../photomatch/docs/licences.md`.
