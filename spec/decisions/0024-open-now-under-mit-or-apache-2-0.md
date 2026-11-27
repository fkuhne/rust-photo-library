# 0024. Open now, under MIT OR Apache-2.0

- Status: Accepted
- Date: 2026-11-27
- Supersedes: [0003](0003-private-now-written-so-it-can-be-opened.md)

## Context

Record 0003 kept the repository private, written so that it could be opened, the decision to open it left for a record of its own. PhotoMatch's owner on 2026-11-27, as the apps' shared design system, `react-photo-ui`, was being set up: "I would make both the ui repo and the rust library open now, as a way to share what I am doing... considering that recruiter can look at my github". The owner's picks that day, of the choices laid out:
- **The licence:** "MIT OR Apache-2.0, both", for this repository and `react-photo-ui`.
- **The history:** "Keep history as it is": the commits keep their author's address; rewriting them would change every commit's id, which PhotoMatch's past commits pin.
- **First:** "A reader's README first", then the owner looks, then the repository goes public.

What was read and checked on 2026-11-27:
- **The Rust API Guidelines,** C-PERMISSIVE: the Rust project "is dual-licensed, under either the MIT or Apache 2.0 licenses. Crates that simply need the maximum compatibility with the Rust ecosystem are recommended to do the same", with `license = "MIT OR Apache-2.0"`, the files `LICENSE-APACHE` and `LICENSE-MIT` in the root, and a License section in the README; "a permissively-licensed crate should generally only depend on permissively-licensed crates."
- **The Cargo Book:** `license` takes an SPDX expression, `OR` letting the user choose either; `publish = false` keeps a package off crates.io.
- **GitHub's "Setting repository visibility":** made public, "The code will be visible to everyone", "Anyone can fork your repository", "Your changes will be published as activity", and stars and watchers are erased.
- **GitHub's deploy keys:** one "grants access to a single repository"; a public repository needs none.
- **What the repository holds:** 119 files; no file path, key or token in any version of any file, by `git log --all -p` searched for paths, addresses and key and token patterns; 40 commits, all by the owner, with the owner's address. Its dependencies' licences are listed in [`docs/inventory.md`](../../docs/inventory.md): all MIT, Apache-2.0 or both, but LibRaw, CDDL 1.0 or LGPL 2.1, used under CDDL 1.0, as a submodule of LibRaw's own public repository; the Focus Points plugin's maps, Apache 2.0, their licence beside them (record 0022); GeoNames' places, CC BY 4.0, which asks for credit (record 0010).

## Decision

**The repository is public on GitHub, licensed MIT OR Apache-2.0 at the user's option, its history as it is; it stays off crates.io for now.**

- **The licence:** `license = "MIT OR Apache-2.0"` in `Cargo.toml`; `LICENSE-MIT` and `LICENSE-APACHE` in the root, Apache's text as apache.org gives it; a License and a Contribution section in the README, as the guidelines lay out. The vendored `rsraw` and `rsraw-sys` keep their own MIT licence, and LibRaw, the Focus Points maps and GeoNames' places theirs, each credited in the README.
- **A README for readers:** what the library is and does, which apps use it, how to build and test it, and the credits.
- **`publish = false` stays:** published on crates.io later, if others come to use it.
- **The rules of record 0003 stay:** every dependency allows MIT or Apache 2.0 publishing and closed use, nothing from GPL or AGPL code, and no secret or private path in the code.
- **The apps reach it over HTTPS,** once public, with no key: a change in each app, in its own steps.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Private, as record 0003 had it | The owner wants the work seen |
| MIT alone, or Apache-2.0 alone | The owner's pick is the dual licence, which the Rust guidelines recommend |
| Its history rewritten, the commits' address replaced | Every commit's id would change, and PhotoMatch's past commits pin the library by them |
| Published on crates.io now | It serves three apps of the owner's; documentation for others comes first |

## Consequences

- **The code, the spec, the records and the notes are public,** PhotoMatch's plans as they read in them among them; PhotoMatch itself stays closed (its record 0017).
- **The owner's address is on the public commits.** New commits may use GitHub's noreply address, if the owner sets it.
- **No deploy key** is needed by the apps' CI, once they reach it over HTTPS.

## Assumptions to verify

- [x] The repository public on GitHub, at the owner's word after the README: on 2026-11-27, `gh repo view` says `PUBLIC`, its licence read by GitHub as Apache 2.0, one of the two; no fork or star before.
- [ ] PhotoMatch builds from it over HTTPS with no key, in its own step.

## Sources

- PhotoMatch's owner, on 2026-11-27.
- The Rust API Guidelines, Necessities; The Cargo Book, the manifest format; GitHub's "Setting repository visibility" and "Managing deploy keys"; read on 2026-11-27.
- [`docs/inventory.md`](../../docs/inventory.md); records 0003, 0010 and 0022; `git log --all -p`, searched on 2026-11-27.
