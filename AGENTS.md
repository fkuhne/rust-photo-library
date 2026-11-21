# AGENTS.md

Instructions for coding agents working in this repository. Keep this file short: detail belongs in `spec/` and `docs/`, read when a step needs it.

## What this is

rust-photo-library is the Rust library, the crate `photo-library`, that the owner's photo apps share: PhotoMatch, PhotoDev, and next PhotoBook. It holds:
- what they had been copying from one another: reading RAW and image files, HEIC and AVIF through the OS, thumbnails, LibRaw's build with one `rsraw`, folder listing;
- the ratings that travel in XMP sidecars.

It starts from PhotoMatch's code, as it is ([0004](spec/decisions/0004-photomatch-s-code-as-it-is-is-the-starting-point.md)). Each app depends on it at a pinned commit ([0002](spec/decisions/0002-one-rust-crate-in-its-own-repository-pinned-by-each-app.md)). It's private for now, written so it can be opened ([0003](spec/decisions/0003-private-now-written-so-it-can-be-opened.md)).

Who uses it:
- **The owner** builds the apps, and is the person you work with.
- **The apps' own code** calls the library. A change here is a change in every app that moves its pin, so behaviour changes are steps of their own.

What the project covers is in [`spec/product.md`](spec/product.md). What each app had copied, and the licences of what moves in, with sources, is in [`docs/inventory.md`](docs/inventory.md).

## Read before acting

1. [`spec/product.md`](spec/product.md): what we do and what we don't. It's the source of truth for scope.
2. [`spec/requirements.md`](spec/requirements.md): each requirement, how it's met and how it's verified.
3. [`spec/plan.md`](spec/plan.md): the steps, in order. The next step is the first unticked one.
4. [`spec/decisions/README.md`](spec/decisions/README.md): the decision records that touch the step.
5. [`docs/inventory.md`](docs/inventory.md): the notes, for the sections the step relies on.

## How we work

- **One plan step at a time.** Before a step with design choices, lay out the options, the evidence and a recommendation, and let the owner pick. Don't decide a whole step alone.
- **Show evidence the owner can see:** real output and real numbers, such as scores, time, peak memory and disk use. Not just "it ran".
- **Write down what a step measures,** in `docs/`, with its command, its settings and the machine it ran on.
- **Check the documentation before recommending.** Before any recommendation, read the documentation and the guidelines of the developers and the community for each technology involved, such as a language's API guidelines or a tool's reference, and cite what was read. Memory alone isn't enough; what couldn't be checked is marked unchecked. A strong rule of the owner's, for every project.
- **Source every claim in the notes:** a paper's section or table, a model card, a config, the code, a vendor page, or our own run. A number worked out but not yet run is marked "computed". A claim from memory is marked "unchecked".
- **A record for every decision**, from [`spec/decisions/decision-template.md`](spec/decisions/decision-template.md), committed on its own or with the change it justifies.
  - Proposed becomes Accepted in the commit that implements it.
  - A reversed decision gets a new record, and the old one is marked superseded.
- **Guesses go under "Assumptions to verify"**, ticked with evidence once checked.
- **Small commits.** Each one is a coherent increment, and none references files that only arrive later. Tick the plan step in the commit that finishes it.
- **A change of scope starts in `spec/product.md`.** When the work departs from the plan, update the plan in the same change.
- **After a step,** explain what was built or learned and why, with links to the lines, before moving on.
- **Skills** run these routines:
  - [`next-step`](.agents/skills/next-step/SKILL.md) prepares the plan's next step;
  - [`decide`](.agents/skills/decide/SKILL.md) writes a record.

  They live in `.agents/skills/`; `.claude/skills/` points Claude Code to them.

## Commands

The workspace is the crate `photo-library` at the root, and the vendored `rsraw` and `rsraw-sys` in `vendor/`, with LibRaw 0.22.3 as `rsraw-sys`'s submodule ([0005](spec/decisions/0005-a-workspace-of-three-packages-libraw-always-built-photomatch-s-modules.md)). It needs Rust (through rustup), a C and C++ compiler, and libclang for `bindgen`: on a Mac, Xcode's Command Line Tools.

```sh
git submodule update --init      # once, after cloning: LibRaw
cargo test                       # build, LibRaw included, and run the tests: the moved ones, and tests/public_api.rs, the names the apps use
cargo build --release            # the library alone
cargo run --example list_folder -- <folder>   # a folder's photos as the library lists them
cargo test --features places     # the tests of the place names too (record 0010)
cargo run --release --features places --example place_of -- <folder>   # where each photo was taken, named
python3 -I scripts/places-data.py <folder> data/places.tsv.gz   # the place list again, from GeoNames' files in <folder>
```

## Conventions

- **English everywhere:** code, notes, records.
- **Resources are stated, not assumed.** Each step says what it needs: memory, disk, time and money. Each measurement names the machine it ran on.
- **Dependencies at the versions the apps build with,** as default requirements, and the lockfile committed for the library's own builds and tests: a library's lockfile doesn't reach the apps, whose own lockfiles pin what they build ([0006](spec/decisions/0006-dependencies-as-default-requirements-the-lockfile-for-the-library-s-own-builds.md)).
- **Nothing large in git.** Photos and test files stay out of it: the tests read the apps' test RAWs from outside, as PhotoDev's do (`PHOTODEV_TEST_RAWS`). Report disk use after a step that writes data.
- **No money is spent without a record:** hardware, rented compute and paid APIs.
- **The planning machine is where it's built and tested,** for now: the owner's Mac. GitHub's minutes cost the owner money, and its credits are spent: start no workflow without the owner's word.
- **Moved, not rewritten.** Code comes from PhotoMatch as it is, with its tests; a behaviour change is a step of its own, with a record ([0004](spec/decisions/0004-photomatch-s-code-as-it-is-is-the-starting-point.md)).
- **Each app is changed in its own repository,** with a record there, and its own tests and self-check run before its pin moves ([0002](spec/decisions/0002-one-rust-crate-in-its-own-repository-pinned-by-each-app.md)). Check an app's `git status` before touching it: the owner works on the apps in parallel.
- **A local `[patch]` pointing an app at this folder is never committed** in the app ([0002](spec/decisions/0002-one-rust-crate-in-its-own-repository-pinned-by-each-app.md)).
- **Ready to open.** Every dependency must allow MIT or Apache 2.0 publishing and closed use; list it in the inventory as it comes in. No GPL or AGPL code. No secret, path or private name in the code ([0003](spec/decisions/0003-private-now-written-so-it-can-be-opened.md)).
- **Windows and macOS,** both, as the apps are. Code for one OS stays small and marked.
