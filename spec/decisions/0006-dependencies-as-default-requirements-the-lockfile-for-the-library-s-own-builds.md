# 0006. Dependencies as default requirements, at the apps' versions; the lockfile for the library's own builds

- Status: Accepted
- Date: 2026-11-21

## Context

[`AGENTS.md`](../../AGENTS.md) carried the convention of the owner's apps: "Pin dependencies to exact versions, with a lockfile." Plan step 1.2 brings in the first dependencies of the library's own code: `image`, `jpeg-decoder`, `kamadak-exif`, `serde`, `serde_json`, `blake3` and, on Windows, `windows`. Checked against Cargo's documentation on 2026-11-21, as the owner asks before any recommendation:
- **A library's lockfile doesn't reach the apps.** The Cargo FAQ, "Why have `Cargo.lock` in version control?": "`Cargo.lock` does not affect the consumers of your package, only `Cargo.toml` does that."
- **Exact requirements can't be met together.** The Cargo book, specifying dependencies: "When in doubt, use the default version requirement operator", and exact `=` requirements are for cases such as pre-releases. The library and an app each asking for a different exact version of one crate, `serde` for instance, could leave Cargo with no version to choose.
- **Committing the lockfile:** the Cargo book, "Cargo.toml vs Cargo.lock": "When in doubt, check `Cargo.lock` into the version control system."

## Decision

- **The library names its dependencies with default requirements, at the versions PhotoMatch builds with:** `image = "0.25.10"`, `serde = "1.0.229"`, and so on, with PhotoMatch's features. An app's own lockfile pins what the app builds.
- **The library commits its `Cargo.lock`,** seeded from PhotoMatch's, so its own builds and tests use those very versions.
- **AGENTS.md's convention says so,** for this repository.

## Alternatives considered

| Option | Why we didn't choose it |
|---|---|
| Exact `=` requirements, as the apps' convention read | They could make an app's dependencies unresolvable, and the library's lockfile wouldn't reach the app anyway |
| No lockfile | The library's tests would build with whatever versions are newest that day |

## Consequences

- **An app may build the library with newer compatible versions** than its tests ran with, as its lockfile allows. Each app's own tests and self-check, run before it moves its pin ([0002](0002-one-rust-crate-in-its-own-repository-pinned-by-each-app.md)), cover that.

## Assumptions to verify

- [ ] PhotoMatch resolves the library's requirements with its own lockfile unchanged: step 2.1.

## Sources

- [Cargo FAQ](https://doc.rust-lang.org/cargo/faq.html#why-have-cargolock-in-version-control), [specifying dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html) and ["Cargo.toml vs Cargo.lock"](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html), read on 2026-11-21.
