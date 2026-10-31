# FrameCensus

**Check that a rendered image sequence contains exactly the frames you expected,
and that every frame is structurally complete and consistent with the rest.**

FrameCensus is a planned Rust library and command-line tool. You declare what a
render should have produced: a numbered file pattern, a frame range, optional
views, and required channels. FrameCensus reports every file that is missing,
unexpected, empty, unreadable, truncated, malformed or inconsistent with the
other frames. Reports are available as a human-readable table or as JSON, and
the exit code is suitable for CI and render-farm post-job steps.

> **Status: design approved; not implemented.** There is no checking
> implementation yet. This repository currently contains the package scaffold
> and quality gates, the project description, the
> [architecture design](docs/architecture.md) and the
> [implementation roadmap](docs/roadmap.md). Every API and CLI example below is
> a **proposed API** and does not run today.

## The problem

Renderers, compositors and render farms write numbered image sequences:
thousands of frames per shot, sometimes one file per stereo view, often with
many channels per file. Failures in that output are usually quiet:

- The first or last frame of the range was never written.
- A node crashed mid-write and left an EXR whose pixel data stops early.
- A task "succeeded" but wrote a zero-byte file.
- A restarted task used different settings, so a stretch of frames has a
  different resolution, channel set or compression.
- Stray frames from an earlier, longer render are still in the directory.

Existing open-source sequence tools such as
[fileseq](https://github.com/justinfx/fileseq),
[pyseq](https://github.com/rsgalloway/pyseq) and
[lsseq](https://github.com/jrowellfx/lsseq) *discover* sequences from the files
that exist. They can report gaps inside a detected range, but they cannot know
that frames before the first file or after the last file were expected.
Per-file header tools such as OpenImageIO's
[`iinfo`](https://openimageio.readthedocs.io/en/latest/iinfo.html) or
OpenEXR's `exrheader` read the header and can succeed on a file whose pixel
data is cut short. Full decoders catch damage but read every pixel, which is
slow on large sequences. The usual result is a hand-written script per
pipeline.

FrameCensus closes that gap with an explicit **expected set** as the oracle,
plus inexpensive structural checks that read headers and chunk tables, not
pixels.

## Intended users and workflows

- **Pipeline and render-farm maintainers**: run FrameCensus as a post-job step,
  fail the job and re-queue only the bad frames.
- **Compositors and review coordinators**: check a delivery before dailies or
  hand-off instead of discovering a bad frame during playback.
- **Developers of renderers, plugins and image pipelines**: assert in CI that a
  test render produced the full expected set with stable headers.
- **Archive and delivery QC**: confirm that a transferred sequence is complete
  and structurally intact after copying.

## Example (proposed)

A stereo render, `renders/left/beauty.1001.exr` … `renders/right/beauty.1240.exr`,
should contain frames 1001–1240 for both views with channels R, G, B, A and Z.

**Command line (proposed):**

```console
$ framecensus check 'renders/%V/beauty.####.exr' \
    --frames 1001-1240 --views left,right \
    --require-channels R,G,B,A,Z

VIEW   FRAMES      FINDING        DETAIL
left   1001        missing        expected renders/left/beauty.1001.exr
left   1010        truncated      chunk 61 of 68 extends to byte 9437184; file is 6291456 bytes
left   1012        empty          file is 0 bytes
right  1015-1022   inconsistent   display_window 2048x1080; 470 of 478 readable frames have 1920x1080
right  1241        unexpected     frame not in 1001-1240

480 expected · 479 present · 1 missing · 1 truncated · 1 empty · 8 inconsistent · 1 unexpected
FAIL (exit 1)
```

Add `--output-format json` for a machine-readable report that a farm script can use to
re-queue exactly the failing frames.

**Rust library (proposed):**

```rust
use std::num::NonZeroUsize;

use framecensus::{check, Expectation, Field, Level, Options};

let expectation = Expectation::builder("renders/%V/beauty.####.exr")?
    .frames("1001-1240".parse()?)
    .views(["left", "right"])
    .require_channels(["R", "G", "B", "A", "Z"])
    .consistent([Field::DisplayWindow, Field::DataWindow, Field::Channels])
    .build()?;

let jobs = NonZeroUsize::new(16).expect("16 is non-zero");
let report = check(&expectation, &Options::default().level(Level::Structure).jobs(jobs))?;

for finding in report.findings() {
    println!("{:?} {:?} {}", finding.view(), finding.frame(), finding.kind());
}
if report.outcome().is_fail() {
    std::process::exit(1);
}
```

## Planned capabilities

Initial release:

- Frame ranges in a subset of the common `fileseq` notation (`1001-1240`,
  `1-100x2`, `1,5,10-20`, negative frames).
- Numbered path patterns using `#` / `@` padding or `%04d`, with `%V` / `%v`
  view tokens.
- Expected-set findings: **missing**, **unexpected**, **padding mismatch**,
  **duplicate**, **empty**, **below minimum size**, **not a regular file**,
  **unreadable**.
- Format checks for **OpenEXR** (single-part and multipart, scanline and tiled)
  and **PNG**: wrong format, header errors, **truncated** data, **malformed**
  offset or chunk tables, trailing data.
- Required EXR channels.
- Cross-frame consistency of selected header fields (display window, data
  window, channels and pixel types, compression, PNG size and pixel format),
  either against declared values or against the majority of frames.
- Parallel scanning with a bounded worker count and bounded memory per file.
- JSON report with a versioned schema, a compact human table that collapses
  runs of frames, and stable exit codes.
- A file `inspect` command that prints what FrameCensus sees in one file.

Planned later (see the architecture's deferred list): full pixel decoding as an
opt-in check level, multiview EXR stored inside one file, deep EXR structure,
DPX and TIFF, an expectation file with multiple sequences, a settle-time policy
for files still being written, and JUnit or SARIF output.

## Non-goals

- Visual regression or image comparison between frames or against references.
- Temporal checks such as flicker or drift detection.
- Render scheduling, re-queueing or farm management. FrameCensus reports; the
  caller decides what to re-run.
- Repairing or rewriting files.
- Proving a file is free of corruption. Structural checks detect truncation and
  broken tables; they do not prove that compressed pixel data decodes
  correctly. That needs the planned opt-in decode level.

## Technical overview

FrameCensus is planned as one Rust package with a library and a CLI binary.
A check runs in four stages:

1. **Expect**: parse the pattern and frame set into an expected set of
   `(view, frame)` keys, without materialising huge ranges.
2. **Discover**: list each target directory once and match file names against
   the compiled pattern, then compare found keys with expected keys.
3. **Inspect**: in a bounded worker pool, stat each present file and read only
   its header and chunk tables. EXR metadata comes from the
   [`exr`](https://crates.io/crates/exr) crate; the chunk-table walk follows the
   [OpenEXR file layout](https://openexr.com/en/latest/OpenEXRFileLayout.html).
   PNG files are walked chunk by chunk per the
   [PNG specification](https://www.w3.org/TR/png-3/).
4. **Compare and report**: group header fields across frames, emit typed
   findings with evidence, sort them deterministically, and compute the
   outcome.

Per-file problems are always findings, never errors that abort the run. The
tool never writes to the scanned directories, and it treats every scanned file
as untrusted input.

The minimum supported Rust version is 1.85, with
`#![forbid(unsafe_code)]`, `clippy -D warnings`, `rustfmt` and `cargo-deny` in
CI; see [docs/development.md](docs/development.md) for the local quality gates.

## Design

The full design, including the domain model, check levels, finding codes,
JSON schema, testing strategy, initial slice and open questions, is in
[docs/architecture.md](docs/architecture.md).

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
