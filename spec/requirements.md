# Requirements

Each row comes from [`product.md`](product.md) and says how it will be met and how we'll know. A row isn't traded away for convenience. If one has to change, change `product.md` first.

"How it's met" names the plan step or the decision while the row is still to build. Once built, it names the code.

| # | Requirement | How it's met | How it's verified |
|---|---|---|---|
| R1 | One crate, `photo-library`, builds and tests on the owner's Mac, LibRaw included | [0002](decisions/0002-one-rust-crate-in-its-own-repository-pinned-by-each-app.md); step 1.1 | `cargo test --release` passes in the library |
| R2 | Photos read as in PhotoMatch: RAW through LibRaw, image files, HEIC and AVIF through the OS | [0004](decisions/0004-photomatch-s-code-as-it-is-is-the-starting-point.md); step 1.2 | PhotoMatch's own tests pass in the library; the samples' thumbnails and details come out as before |
| R3 | Thumbnails and their cache, as in PhotoMatch | Step 1.2 | A sample folder's thumbnails, made by PhotoMatch on its copy and on the library, are the same bytes |
| R4 | One `rsraw`, with both apps' additions, and LibRaw 0.22.3 pinned | Steps 1.1 and 1.2 | Both apps build and pass their tests with it |
| R5 | Folder listing, a RAW+JPEG pair shown once | Step 1.3 | Its tests, from PhotoDev and PhotoMatch |
| R6 | Ratings in XMP sidecars by PhotoMatch's rules: only its fields changed, all else kept, the newer change winning | Step 1.4 | PhotoMatch's sidecar tests pass in the library; a sidecar holding Lightroom's edits keeps them |
| R7 | PhotoMatch on the library, behaving as before | Step 2.1 | Its tests and self-check pass; thumbnails and sidecars the same as on its copy |
| R8 | PhotoDev on the library | Step 2.2 | Its tests, self-checks and originals test pass |
| R9 | No library code copied in either app | Steps 2.1 and 2.2 | The copied files are gone from both, by `git ls-files` |
| R10 | Ready to open: every line and every dependency allows MIT or Apache 2.0 publishing and closed use | [0003](decisions/0003-private-now-written-so-it-can-be-opened.md); each step | [`docs/inventory.md`](../docs/inventory.md) lists each dependency's licence, reviewed at the end of each phase |
| R11 | Every claim in the notes names its source | `docs/` cites the source and the date. A computed number is marked "computed", and a claim from memory "unchecked" | Reviewed at the end of each phase |
| R12 | Every measurement is written down with how it was run | The step's notes in `docs/` | Each number in `docs/` names its command, its settings and the machine |
| R13 | No money is spent without a record | A record for each purchase, rental or paid API | Each cost appears in a record |
| R14 | Every decision has a record | `spec/decisions/` ([0001](decisions/0001-work-from-a-spec-and-record-every-decision.md)) | Reviewed at the end of each phase |
| R15 | Any coding agent can pick the work up | `AGENTS.md`, which `CLAUDE.md` and `GEMINI.md` import; the `next-step` and `decide` skills | A new session finds the next step and its context from the repository alone |
