# 0001. Work from a spec and record every decision

- Status: Accepted
- Date: 2026-11-20

## Context

The owner works on rust-photo-library, a Rust library shared by the owner's photo apps, with a coding agent, across many sessions. A session ends, and with it the conversation. That conversation held what was learned, measured and decided, and why. A new session then starts from nothing.

The owner already works from a spec in their other projects, such as `photomatch`, `photodev`, `photobook`, `ai-bootstrap`, `ultra-bert` and `idea-lab`:
- a product spec as the source of truth for scope;
- a table from each requirement to how it's met and verified;
- a plan of small steps;
- a record for each decision.

The library was decided in PhotoDev's repository on 2026-11-20 (PhotoDev's `docs/app.md`, step 2.1, and the conversation that followed it). Its three users, the apps, are built this way too.

## Decision

The project works from a spec. Everything that drives the work lives in `spec/`:
- `spec/product.md`: the problem, who it's for, the scope and the non-goals. The source of truth.
- `spec/requirements.md`: each requirement, how it's met, and how it's verified.
- `spec/plan.md`: phases of small steps, each ticked in the commit that finishes it.
- `spec/decisions/`: one record per decision, numbered and made from `decision-template.md`, with an index.

`docs/` holds what the project produces for a reader. Its first page is the inventory, [`docs/inventory.md`](../../docs/inventory.md): what each app had copied, where it came from, and how the copies differ.

The method's routines run as skills, kept in `.agents/skills/`: `next-step` and `decide`. `AGENTS.md` tells any agent how to work, and `CLAUDE.md` and `GEMINI.md` import it.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Notes in one README | It mixes the plan, the decisions and the findings, and it grows without structure. |
| The library inside one app's repository, with that app's spec | Its decisions would mix with the app's, and the other apps would depend on that app's whole repository. |

## Consequences

- An agent reads `spec/` and the notes before it acts, as `AGENTS.md` says.
- A change that touches an app also gets a record in that app's repository, for what it means there.

## Assumptions to verify

- [ ] The method fits a library. Check at the end of phase 1, and adapt `AGENTS.md` if it doesn't.

## Sources

- The `new-project` skill (`~/.claude/skills/new-project/`), from which this repository was started.
- `../photodev/spec/decisions/0001-work-from-a-spec-and-record-every-decision.md`, the same method for PhotoDev.
