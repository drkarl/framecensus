# FrameCensus

**Check that a rendered image sequence contains exactly the frames you expected,
and that every frame is structurally complete and consistent with the rest.**

FrameCensus is a planned Rust library and command-line tool for checking
rendered image sequences against an explicit expected set. The design is
approved; **no checking functionality is implemented yet.** This repository
currently contains the package scaffold, the
[architecture design](docs/architecture.md) and the
[implementation roadmap](docs/roadmap.md).

## Purpose

Renderers, compositors and render farms write numbered image sequences:
thousands of frames per shot, sometimes one file per stereo view, often with
many channels per file. Failures in that output are usually quiet:

- The first or last frame of the range was never written.
- A node crashed mid-write and left an EXR whose pixel data stops early.
- A task "succeeded" but wrote a zero-byte file.
- A restarted task used different settings, so a stretch of frames has a
  different resolution, channel set or compression.
- Stray frames from an earlier, longer render are still in the directory.

Existing sequence tools *discover* sequences from the files that exist. They
can report gaps inside a detected range, but they cannot know that frames
before the first file or after the last file were expected. Per-file header
tools can succeed on a file whose pixel data is cut short, and full decoders
catch damage only by reading every pixel.

FrameCensus is designed to close that gap with an explicit **expected set** as
the oracle, plus inexpensive structural checks that read headers and chunk
tables, not pixels. Reports will be available as a human-readable table or as
JSON, with exit codes suitable for CI and render-farm post-job steps.

## Intended users

- **Pipeline and render-farm maintainers**: run FrameCensus as a post-job step,
  fail the job and re-queue only the bad frames.
- **Compositors and review coordinators**: check a delivery before dailies or
  hand-off instead of discovering a bad frame during playback.
- **Developers of renderers, plugins and image pipelines**: assert in CI that a
  test render produced the full expected set with stable headers.
- **Archive and delivery QC**: confirm that a transferred sequence is complete
  and structurally intact after copying.

These are the intended users of the planned tool. FrameCensus has no users or
releases yet.

## Status

This is a scaffold. The package builds, tests, documents and lints cleanly, but
the library has no public checking API and the binary only prints usage. No
frame, file or sequence checking is implemented, and nothing is published.

The planned capabilities, domain model, check levels, finding codes, JSON
schema and testing strategy are described in
[docs/architecture.md](docs/architecture.md); the implementation order is in
[docs/roadmap.md](docs/roadmap.md).

## Technology

FrameCensus is a single Rust package with a library and a `framecensus`
binary:

- Rust 2024 edition, minimum supported Rust version 1.85.
- The binary and its `clap` dependency sit behind the default `cli` feature;
  the library builds without the CLI.
- Planned runtime crates: `exr` and `png` for format structure, `rayon` for a
  bounded worker pool, `serde` and `serde_json` for reports, `thiserror` for
  errors, and `libc` on Unix for the non-blocking open flag.
- Quality gates: `rustfmt`, `clippy` with warnings denied, `cargo doc` with
  warnings denied, and `cargo deny` for licences, advisories, sources and
  duplicate versions. Library code denies `unwrap`, `expect` and `panic`.
- CI runs the quality gate on Linux and tests on Linux, macOS and Windows with
  stable Rust, plus Linux on Rust 1.85.

## Build and run

Prerequisites: Rust through `rustup` (the pinned toolchain in
`rust-toolchain.toml` includes `rustfmt` and `clippy`) and `cargo-deny` 0.19.

```console
$ cargo build
$ cargo test
$ cargo run -- --help
$ ./scripts/check.sh
```

`cargo run -- --help` prints the command usage; running the binary with no
arguments also prints usage. The binary does not inspect any files yet.
`./scripts/check.sh` runs the full local quality gate. See
[docs/development.md](docs/development.md) for each gate and the MSRV test.

## Non-goals

- Visual regression or image comparison between frames or against references.
- Temporal checks such as flicker or drift detection.
- Render scheduling, re-queueing or farm management. FrameCensus reports; the
  caller decides what to re-run.
- Repairing or rewriting files.
- Proving a file is free of corruption. Structural checks detect truncation and
  broken tables; they do not prove that compressed pixel data decodes
  correctly.

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
