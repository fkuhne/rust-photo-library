# rust-photo-library

The Rust library, the crate `photo-library`, shared by the owner's photo apps: PhotoMatch, PhotoDev, and next PhotoBook. It holds what they had been copying from one another:
- reading RAW files through LibRaw, image files, and HEIC and AVIF through the OS;
- thumbnails cached on disk;
- LibRaw's build, with one `rsraw`;
- folder listing;
- ratings in XMP sidecars.

It's private for now, written so that it can be opened later. The project is at the spec stage.

## Where to start

- [`docs/inventory.md`](docs/inventory.md): what each app had copied, how the copies differ, and the licences of what moves in, each claim with its source.
- [`spec/product.md`](spec/product.md): what this project covers and what it doesn't.
- [`spec/plan.md`](spec/plan.md): the steps, in order. The next one is the first unticked.
- [`spec/decisions/`](spec/decisions/README.md): why each choice was made.

## How it's worked on

From a spec, one plan step at a time, with a record for each decision, the same way as the owner's other projects. [`AGENTS.md`](AGENTS.md) gives the method to coding agents.
