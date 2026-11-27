# rust-photo-library: product spec

This is the source of truth for what the project covers and what it doesn't. A change of scope starts here, then flows into [`requirements.md`](requirements.md), [`plan.md`](plan.md) and the [decision records](decisions/README.md).

> **Drafted on 2026-11-20,** from the owner's conversation in PhotoDev's repository, where the library was decided. The name is the owner's: `rust-photo-library` for the repository, and `photo-library` for the crate, by Rust's custom. Two answers are defaults proposed the same day, marked below: when to open the source, and when PhotoBook moves. The owner reviews them in plan step 0.2.

rust-photo-library is a Rust library, the crate `photo-library`, shared by the owner's photo apps: PhotoMatch, PhotoDev and PhotoBook. It holds the code they had been copying from one another: reading photos, making thumbnails, building LibRaw, and the ratings that travel in XMP sidecars.

## The problem

The owner's three photo apps are built with the same stack, Tauri 2 with a Rust core. Each one copied what it needed from PhotoMatch, the first, piece by piece (PhotoDev's record 0008, PhotoBook's record 0008). By 2026-11-20 that meant:
- **PhotoMatch's library code:** 2,697 lines of Rust, in nine files ([`docs/inventory.md`](../docs/inventory.md)):
  - reading RAW and image files, with HEIC and AVIF through the OS;
  - thumbnails cached on disk;
  - problems as kinds;
  - the ratings, and the XMP sidecars that carry them.
- **PhotoDev's copy** of seven of those files, 1,668 lines.
- **PhotoBook's own photo module,** 338 lines, derived from PhotoMatch's.
- **Two patched copies of `rsraw`,** the Rust wrapper of LibRaw, one in PhotoMatch and one in PhotoDev, each with its own additions, and two copies of LibRaw's build.

Copying costs three ways:
- **The copies drift,** and a fix made in one app doesn't reach the others. PhotoDev found a crash in how it answers thumbnail requests, in code shaped as PhotoMatch's (PhotoDev's `docs/app.md`, step 2.1). PhotoMatch probably has it too (unchecked).
- **Two apps now write the same sidecars.** PhotoDev rates, filters and sorts as PhotoMatch does, sharing a RAW file's ratings through its XMP sidecar (PhotoDev's record 0021). That's only safe if both apps follow exactly the same rules, and copies can drift apart.
- **Each copy is more code to build and test,** LibRaw's build above all.

On 2026-11-20 the owner agreed to share the code instead: "yes, let's create this shared lib. we can even think in distributing it as an open source library or something that others can reuse it."

## Who it's for

1. **The owner's photo apps:** PhotoMatch and PhotoDev first, then PhotoBook.
2. **The owner,** who builds them: one place to fix, test and extend what they share.
3. **Later, perhaps, other developers,** if it's opened as open source: "something that others can reuse", in the owner's words.
4. **A coding agent** working with the owner, who needs the spec, the notes and the records to pick up where the last session stopped.

## What it is

One Rust library, a crate, in its own repository ([0002](decisions/0002-one-rust-crate-in-its-own-repository-pinned-by-each-app.md)):
- **Photo reading:** RAW files through LibRaw, image files through the `image` crate, and HEIC and AVIF through the OS's own decoders, behind one interface.
- **Thumbnails,** from a RAW file's embedded preview or the image itself, cached on disk.
- **LibRaw's build:** LibRaw, in C++, pinned as a git submodule and compiled from source, and one `rsraw` with both apps' additions. The library's own code and its interface are Rust; LibRaw and the OS's decoders are what it calls.
- **Folder listing,** with a RAW+JPEG pair shown once.
- **Ratings and their XMP sidecars:** pick, stars and label, by PhotoMatch's rules (PhotoMatch's record 0014). Only those fields are changed, all else in a sidecar is kept, and the newer change wins.
- **Problems as kinds,** for each app to word in its own interface.

Each app depends on it at a pinned commit. While a change is under way, it depends on the local folder on the owner's Mac, so the change is tested in every app before any moves to it.

It starts from PhotoMatch's code, as it is, so that PhotoMatch's behaviour doesn't change ([0004](decisions/0004-photomatch-s-code-as-it-is-is-the-starting-point.md)).

## Outcomes

- **No library code copied in PhotoMatch or PhotoDev:** each depends on `photo-library` for photo reading, thumbnails, LibRaw, folders and ratings.
- **PhotoMatch behaves as before:** its tests and self-check pass on the library, and its thumbnails and sidecars come out the same.
- **PhotoDev's tests and self-checks pass on it,** its originals test included.
- **A fix lands once:** one change in the library reaches both apps, with a commit pin moved in each.
- **Both apps write a shared sidecar by the same code,** and a photo rated in either shows the same in the other.
- **Open:** public on GitHub, MIT OR Apache-2.0, every line and every dependency's licence allowing it, each credited ([0024](decisions/0024-open-now-under-mit-or-apache-2-0.md), superseding [0003](decisions/0003-private-now-written-so-it-can-be-opened.md)).

## Scope

**Version 1,** from the conversation of 2026-11-20:
- photo reading, thumbnails and their cache, problems as kinds;
- LibRaw's build, with one `rsraw` holding both apps' additions;
- folder listing;
- the ratings model and the XMP sidecar rules;
- PhotoMatch and PhotoDev moved to it.

**Next:**
- **PhotoBook moved to it.** *A default proposed on 2026-11-20, for the owner to confirm:* after PhotoMatch and PhotoDev, to keep version 1 small.
- **CI,** building and testing the library on macOS and Windows, once there are minutes for it.

**Later, as ideas:**
- **On crates.io,** with its documentation and examples, for others to reuse: from the owner on 2026-11-20. Public on GitHub since 2026-11-27 ([0024](decisions/0024-open-now-under-mit-or-apache-2-0.md)).
- **More shared pieces,** when two apps need them: PhotoDev's colour maths and ICC profiles, or its edit settings, if PhotoMatch or PhotoBook comes to need them.

## Non-goals

- **User interfaces:** each app keeps its own, React and TypeScript.
- **What makes each app itself:** PhotoDev's colour and tone engine, PhotoMatch's taste learning and culling, PhotoBook's print layouts. They stay in their apps, unless a later decision shares one.
- **A general photo library for the world,** for now: it serves the three apps first. Opening it doesn't change what it does.

## Constraints

- **PhotoMatch is released.** Its behaviour mustn't change with the move: the same thumbnails, the same sidecars, the same problems.
- **Licences:** the apps are closed, so that they can be sold. The library must allow use in a closed, paid app, and be ready to publish under MIT or Apache 2.0.
  - **Its own code** is the owner's.
  - **`rsraw` and `rsraw-sys`** are MIT.
  - **LibRaw** is used under CDDL 1.0, as PhotoMatch does (its record 0013).
  - **Every other dependency's licence** is checked as it comes in.
- **Built and tested on the owner's Mac.** Apple M3, 8 GiB, macOS 14.3. The owner's GitHub Actions credits are spent, so CI waits (PhotoDev's record 0015). Windows is checked when the owner can.
- **Resources are stated, not assumed.** Each step says what it needs: memory, disk, time and money. Each measurement names the machine it ran on.
- **Money:** nothing is spent without a record.
- **Nothing large in git.** Photos and test files stay out of it; the tests use the apps' test RAWs from outside the repository.
- **English** everywhere: code, notes and records.

## Open questions

For the owner, in step 0.2:
1. **The crate's name on crates.io,** if it's opened: `photo-library` must be free there (unchecked). The repository's name, `rust-photo-library`, is the owner's, from 2026-11-20.
2. **When to open it:** answered on 2026-11-27, now, MIT OR Apache-2.0 ([0024](decisions/0024-open-now-under-mit-or-apache-2-0.md), superseding [0003](decisions/0003-private-now-written-so-it-can-be-opened.md)).
3. **PhotoBook:** next, after PhotoMatch and PhotoDev, is the default.
4. **CI credentials:** a read-only deploy key from 2026-11-22; once the repository is public, none, the apps reaching it over HTTPS ([0024](decisions/0024-open-now-under-mit-or-apache-2-0.md)).
