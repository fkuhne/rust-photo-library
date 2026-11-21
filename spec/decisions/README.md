# Decision records

This folder records every decision that shapes the project, one file per decision. Each record states:
- the context;
- what we chose;
- the alternatives, and why we rejected them;
- the consequences;
- the assumptions we still have to verify.

**Statuses**
- **Proposed**: planned, but not yet confirmed by the owner or proven by an implementation. The commit that implements it flips it to Accepted, amending it if the implementation taught us something.
- **Accepted**: in force.
- **Superseded**: replaced by a later record, which it links to. It's kept for history.

**Adding a record**
1. Copy [`decision-template.md`](decision-template.md) to the next free number, e.g. `0002-short-title.md`.
2. Fill it in and add a row to the table below.
3. Commit it on its own, before or together with the change it justifies.

Don't rewrite a record to change its decision. Write a new record and mark the old one superseded. Ticking assumptions and adding evidence is fine. Never renumber: commits and other records cite records by number.

| # | Decision | Status | Date |
|---|---|---|---|
| [0001](0001-work-from-a-spec-and-record-every-decision.md) | Work from a spec and record every decision | Accepted | 2026-11-20 |
| [0002](0002-one-rust-crate-in-its-own-repository-pinned-by-each-app.md) | One Rust crate in its own repository, pinned by each app | Proposed | 2026-11-20 |
| [0003](0003-private-now-written-so-it-can-be-opened.md) | Private now, written so it can be opened | Proposed | 2026-11-20 |
| [0004](0004-photomatch-s-code-as-it-is-is-the-starting-point.md) | PhotoMatch's code, as it is, is the starting point | Proposed | 2026-11-20 |
| [0005](0005-a-workspace-of-three-packages-libraw-always-built-photomatch-s-modules.md) | A workspace of three packages, LibRaw always built, and PhotoMatch's modules as the API | Accepted | 2026-11-21 |
| [0006](0006-dependencies-as-default-requirements-the-lockfile-for-the-library-s-own-builds.md) | Dependencies as default requirements, at the apps' versions; the lockfile for the library's own builds | Accepted | 2026-11-21 |
| [0007](0007-one-folder-listing-from-both-apps-hidden-files-left-out.md) | One folder listing from both apps, hidden files and folders left out | Accepted | 2026-11-21 |
