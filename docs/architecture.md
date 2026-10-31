# FrameCensus architecture

Status: **proposed, awaiting review.** Nothing described here is implemented.
Names, signatures and formats are proposals and may change during review.

## Contents

1. [Problem statement and scope](#1-problem-statement-and-scope)
2. [Requirements](#2-requirements)
3. [Public API and CLI](#3-public-api-and-cli)
4. [Domain model and invariants](#4-domain-model-and-invariants)
5. [Modules and responsibilities](#5-modules-and-responsibilities)
6. [Data flow and lifecycle](#6-data-flow-and-lifecycle)
7. [Algorithms and implementation choices](#7-algorithms-and-implementation-choices)
8. [File formats and compatibility](#8-file-formats-and-compatibility)
9. [Errors, findings and reporting](#9-errors-findings-and-reporting)
10. [Performance and resources](#10-performance-and-resources)
11. [Security and robustness](#11-security-and-robustness)
12. [Dependencies and alternatives](#12-dependencies-and-alternatives)
13. [Testing strategy](#13-testing-strategy)
14. [Initial usable slice](#14-initial-usable-slice)
15. [Deferred functionality and non-goals](#15-deferred-functionality-and-non-goals)
16. [Open technical questions](#16-open-technical-questions)

---

## 1. Problem statement and scope

Render pipelines produce numbered image sequences. A sequence is correct only
if (a) every expected file exists, (b) no unexpected files are mixed in, (c)
each file is a complete, structurally valid image of the expected format, and
(d) the frames agree with each other on the properties that must not change
within a shot (resolution, channels, pixel types).

Sequence discovery tools infer a sequence from the files that exist, so they
cannot report frames missing before the first or after the last file. Header
readers can succeed on files whose pixel data was cut short. Full decoders are
thorough but read every pixel.

FrameCensus validates a sequence against a **declared expectation** and uses
**structural checks** that read headers and chunk tables, not pixel data.

### In scope

- Expected-set validation of numbered file sequences, optionally per view.
- File-level checks: existence, file type, size, readability.
- Format-level structural checks for OpenEXR and PNG.
- Cross-frame header consistency.
- Machine-readable reports and CI exit codes.
- Parallel, bounded scanning of local and network filesystems.

### Out of scope (initial)

Visual regression, temporal comparison, pixel statistics, render scheduling,
farm management, repair. See [section 15](#15-deferred-functionality-and-non-goals).

### What "structural" means

FrameCensus distinguishes three levels of evidence and never reports a
stronger guarantee than the level it ran:

| Level | Reads | Detects | Does not detect |
|---|---|---|---|
| `exists` | directory listing, `stat` | missing, unexpected, empty, wrong file type | anything about content |
| `header` | magic number and header | wrong format, malformed or unsupported header, missing channels, header inconsistency | truncated or damaged pixel data |
| `structure` (default) | header, EXR offset tables and chunk prefixes, PNG chunk headers | truncation, incomplete or out-of-range offset tables, chunk coordinates that do not match the table, trailing data | corrupted bytes inside a chunk of the correct size, wrong pixel values |
| `decode` (deferred) | all pixel data | compression-stream damage, PNG CRC errors | semantically wrong images (black frames, wrong content) |

A file that passes `structure` has the shape of a complete file. It is not
proven to decode. The report states the level used.

---

## 2. Requirements

### Functional

- **F1** Accept an expectation: path pattern with one frame token and optional
  view tokens, a frame set, an optional view list, an image format (inferred
  from the extension or explicit), optional required channels, optional fixed
  header values, consistency fields, and an optional minimum file size.
- **F2** Report each expected `(view, frame)` with no matching file as
  `missing`. If a target directory itself is absent, report one
  `directory_missing` finding instead of one `missing` per frame.
- **F3** Report files that match the pattern but are outside the expected set
  as `unexpected`; files whose digit run violates the padding rule as
  `padding_mismatch`; several files resolving to one key as `duplicate`.
- **F4** Report zero-byte files as `empty`, files smaller than the declared
  minimum as `below_min_size`, non-regular files (directories, FIFOs, sockets,
  dangling links) as `not_a_file`, and I/O failures as `unreadable`.
- **F5** At `header` level and above, report files whose magic number does not
  match the expected format as `wrong_format`, invalid headers as `malformed`,
  valid headers using features FrameCensus cannot inspect as `unsupported`,
  and absent required channels as `missing_channel`.
- **F6** At `structure` level, report files that end before the structure
  requires as `truncated`; invalid offset tables or chunk prefixes as
  `malformed`; bytes after the logical end of the file as `trailing_data`.
- **F7** Compare selected header fields across all readable frames. A field
  with a declared fixed value is compared against it; otherwise frames are
  grouped by value and every frame outside the largest group is reported as
  `inconsistent`, with the group sizes as evidence.
- **F8** Produce a `Report` with a schema version, the effective expectation
  and options, summary counts, findings sorted deterministically, and an
  outcome (`pass` / `fail`) derived from a severity policy.
- **F9** CLI: `check` (validate a sequence), `inspect` (describe individual
  files), human and JSON output, stable exit codes.

### Nonfunctional

- **N1 Correctness over speed.** No finding may claim more than the evidence
  supports. Unsupported features are reported as `unsupported`, never as
  `malformed` and never silently passed.
- **N2 Determinism.** The same filesystem state and inputs produce
  byte-identical JSON regardless of worker count or scheduling. JSON contains
  no wall-clock timestamps by default.
- **N3 Bounded resources.** Memory is O(expected keys + directory entries)
  with a small constant per frame. No single file may cause allocation
  proportional to attacker-controlled header values.
- **N4 Robustness.** No panic on any file content, file name or filesystem
  state. Untrusted input never aborts the run.
- **N5 Read-only.** Never write, rename, lock or delete scanned files. Never
  open non-regular files.
- **N6 Portability.** Linux, macOS and Windows, local disks and network
  filesystems (NFS, SMB). Platform-specific code is limited to one Unix open
  flag (section 11).
- **N7 Rust quality.** Stable Rust, MSRV 1.85 declared in `rust-version`,
  edition 2024, `#![forbid(unsafe_code)]`, `cargo clippy --all-targets
  -- -D warnings`, `cargo fmt --check`, `#![warn(missing_docs)]` with docs on
  every public item, `cargo deny check` (licences, advisories, sources,
  duplicate versions), and no panics on untrusted input. Library code denies
  `clippy::unwrap_used`, `clippy::expect_used` and `clippy::panic`; tests
  are exempt (`allow-unwrap-in-tests` / `allow-expect-in-tests` in
  `clippy.toml`). Internal invariants are expressed through types or
  returned as errors, not asserted with `unwrap`.
- **N8 Small API.** One entry function for checks, one for single-file
  inspection, a builder for expectations, and non-exhaustive enums so new
  finding kinds and fields can be added without breaking callers.

---

## 3. Public API and CLI

All signatures are proposals.

### Library

```rust
// Entry points
pub fn check(expectation: &Expectation, options: &Options) -> Result<Report, Error>;
pub fn inspect_file(path: &Path, options: &InspectOptions) -> FileInspection;

// Expectation
impl Expectation {
    pub fn builder(pattern: &str) -> Result<ExpectationBuilder, Error>;
}
impl ExpectationBuilder {
    pub fn frames(self, frames: FrameSet) -> Self;
    pub fn views<I, S>(self, views: I) -> Self where I: IntoIterator<Item = S>, S: Into<String>;
    pub fn format(self, format: ImageFormat) -> Self;          // default: from extension
    pub fn require_channels<I, S>(self, channels: I) -> Self   // EXR only
        where I: IntoIterator<Item = S>, S: Into<String>;
    pub fn consistent<I: IntoIterator<Item = Field>>(self, fields: I) -> Self;
    pub fn display_window(self, window: IntRect) -> Self;      // fixed value
    pub fn data_window(self, window: IntRect) -> Self;         // fixed value
    pub fn min_size(self, bytes: u64) -> Self;
    pub fn build(self) -> Result<Expectation, Error>;
}

// Frame sets
impl FromStr for FrameSet { type Err = Error; }               // "1001-1240", "1-100x2,200"
impl Display for FrameSet {}                                    // canonical form
impl FrameSet {
    pub fn len(&self) -> u64;
    pub fn contains(&self, frame: i64) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = i64> + '_;
}

// Options
impl Options {
    pub fn level(self, level: Level) -> Self;                  // default Structure
    pub fn jobs(self, jobs: NonZeroUsize) -> Self;             // default: see §10
    pub fn policy(self, policy: Policy) -> Self;               // severities
    pub fn limits(self, limits: Limits) -> Self;               // header bytes, expected keys
    pub fn follow_symlinks(self, follow: bool) -> Self;        // default true
}

// Report
impl Report {
    pub fn findings(&self) -> &[Finding];
    pub fn summary(&self) -> &Summary;
    pub fn outcome(&self) -> Outcome;                          // Pass | Fail
    pub fn files(&self) -> &[FileSummary];                     // per present file
}
impl Finding {
    pub fn kind(&self) -> FindingKind;
    pub fn severity(&self) -> Severity;
    pub fn view(&self) -> Option<&str>;
    pub fn frame(&self) -> Option<i64>;
    pub fn path(&self) -> Option<&Path>;
    pub fn evidence(&self) -> &Evidence;
    pub fn message(&self) -> String;                           // human sentence
}
impl Display for FindingKind {}                                // stable snake_case code, e.g. "truncated"
impl Outcome {
    pub fn is_pass(&self) -> bool;
    pub fn is_fail(&self) -> bool;
}
```

`Report`, `Finding`, `Evidence` and `FileSummary` implement `serde::Serialize`.
The JSON shape is the documented schema (section 8); Rust field names are not
part of that contract.

`check` returns `Err` only when the check cannot be performed as specified (see
section 9). Problems with files are findings.

### CLI

```text
framecensus check <PATTERN> --frames <SET> [options]
    --views <V1,V2,...>             required iff PATTERN contains %V or %v
    --format-hint <exr|png>         override extension-based format
    --require-channels <C1,...>     EXR channels that must exist
    --consistent <F1,...>           fields to compare; default: see §7.5
    --no-consistency                disable cross-frame comparison
    --display-window <WxH|x0,y0,x1,y1>   fixed expected value
    --data-window <x0,y0,x1,y1>          fixed expected value
    --min-size <BYTES>
    --level <exists|header|structure>    default structure
    --jobs <N>
    --deny <KIND,...|warnings>      escalate kinds (or all warnings) to errors
    --allow <KIND,...>              downgrade kinds to warnings
    --output-format <human|json>    default human
    --output <FILE>                 write report to file instead of stdout
    --show-passing                  include per-file summaries in JSON

framecensus inspect <FILE>... [--level ...] [--output-format human|json]
```

Exit codes:

| Code | Meaning |
|---|---|
| 0 | Check ran; no finding at `error` severity. |
| 1 | Check ran; at least one `error` finding. |
| 2 | Check could not run: invalid arguments, pattern or frame set; limits exceeded; report could not be written. |

Human output collapses consecutive frames with the same view, kind and
evidence into one row (`1015-1022`). JSON always lists one finding per file or
key so callers can act on individual frames.

---

## 4. Domain model and invariants

### `Frame`

`i64` frame number. Arithmetic is checked; parsing rejects values outside
±10^15 so formatting and stepping cannot overflow.

### `FrameSet`

A finite set of frames stored as sorted, disjoint, non-adjacent inclusive
intervals `Vec<(i64, i64)>`.

Invariants: every interval has `start <= end`; intervals are sorted by
`start`; for consecutive intervals `a`, `b`: `a.end + 1 < b.start`. The empty
set is representable but `Expectation::build` rejects it.

Stepped input (`1-100x2`) is normalised into unit intervals. `Display` writes
the canonical interval list; it re-detects constant steps when that is shorter
(`1-99x2`). `FromStr(Display(s)) == s` for every set.

### `FramePattern`

A parsed path template: `Vec<Segment>` where
`Segment = Literal(String) | Frame { padding: u8 } | View(ViewForm)` and
`ViewForm = Full | Initial` (`%V` / `%v`).

Invariants:

- Exactly one `Frame` segment, located in the final path component.
- `View` segments only when the expectation declares views; any number of
  them, all substituted with the same view.
- No two token segments are adjacent (a literal separates them), so matching
  is unambiguous.
- `padding` is 1–18.
- Literal segments contain no glob or regex metacharacters with special
  meaning; they are matched byte-for-byte.

### `ViewName`

Non-empty UTF-8 string with no path separator (`/`, `\`), no NUL, not `.` or
`..`. When `%v` is used, initials of declared views must be unique.

### `ExpectedKey`

`(view_index: u16, frame: Frame)`. The expected set is `views × frames`. It is
never materialised as a list: membership is answered by the `FrameSet`.

### `Expectation`

Pattern, frame set, views, `ImageFormat`, required channels, consistency
`FieldSet`, fixed values, minimum size.

Invariants enforced by `build()`:

- Views are non-empty exactly when the pattern contains a view token; views
  are unique.
- `views.len() × frames.len()` does not exceed `Limits::max_expected_keys`
  (default 10 000 000).
- Required channels and EXR-only fields are rejected for PNG.
- A fixed value for a field implies that field is compared.

### `ImageFormat`

`Exr | Png`, non-exhaustive.

### `FileSummary`

Per present file: key, path, size, level reached, and a format-specific
`HeaderSummary` (EXR: per-part name, view attribute, type, data window,
display window, channel list with pixel types and sampling, compression, line
order, tile description, chunk count; PNG: width, height, bit depth, colour
type, interlace). Channel lists are interned (`Arc<ChannelList>`) because
nearly all frames share one.

### `Finding`

`kind`, `severity`, optional key, optional path, typed `Evidence`.

Invariants: findings about a file carry its path and key; `directory_missing`
carries the directory and the view; findings are ordered by
(view index, frame, path, kind order, evidence) in the report.

### `Evidence`

A typed enum, one variant per finding kind, for example:

```rust
#[non_exhaustive]
pub enum Evidence {
    Missing { expected_path: PathBuf },
    Truncated { file_len: u64, required_len: u64, part: u32, chunk: u64, chunk_count: u64 },
    Malformed { reason: MalformedReason, offset: Option<u64> },
    Unsupported { feature: Feature },
    Inconsistent { field: Field, actual: FieldValue, expected: FieldValue,
                   basis: Basis /* Declared | Majority { count, total } */ },
    // ...
}
```

`MalformedReason` is a non-exhaustive enum with stable string codes
(`offset_table_zero_entry`, `offset_out_of_range`, `chunk_coordinate_mismatch`,
`chunk_overlap`, `bad_attribute`, `ihdr_not_first`, `idat_not_contiguous`, …).

### `Severity` and `Policy`

`Severity = Error | Warning | Info`. `Policy` maps `FindingKind` to severity.
Default:

| Kind | Default severity |
|---|---|
| `missing`, `directory_missing`, `empty`, `below_min_size`, `not_a_file`, `unreadable`, `wrong_format`, `truncated`, `malformed`, `missing_channel`, `duplicate` | error |
| `inconsistent` | error for windows, channels, pixel types, part layout, PNG size and pixel format; warning for compression and line order |
| `unexpected`, `padding_mismatch`, `trailing_data`, `unsupported` | warning |

`Outcome::Fail` iff any finding has `Error` severity.

---

## 5. Modules and responsibilities

One package, `framecensus`, with a library and a binary. The binary and its
dependencies sit behind a default `cli` feature so library users can disable
it.

| Module | Responsibility | Depends on |
|---|---|---|
| `frameset` | Range syntax parsing and formatting; interval set operations. | — |
| `pattern` | Pattern parsing; path formatting for a key; file-name matching; padding rule. | `frameset` |
| `expect` | `Expectation`, builder, validation, limits. | `pattern`, `frameset` |
| `discover` | Plan target directories; list each once; match names; classify missing, unexpected, padding, duplicate, directory missing. | `expect`, `pattern` |
| `inspect` | File-type and size checks; magic detection; format dispatch; bounded reader. | `inspect::exr`, `inspect::png` |
| `inspect::exr` | EXR header summary (via `exr`), header-length walk, offset tables, chunk-prefix validation. | `exr` |
| `inspect::png` | PNG signature, IHDR, chunk walk, IEND, trailing data. | — |
| `consistency` | Field extraction, grouping, majority selection, declared-value comparison. | `report` types |
| `report` | `Report`, `Finding`, `Evidence`, `Severity`, `Policy`, `Summary`, ordering, serde schema. | `serde` |
| `scan` | Runs the stages in order; bounded worker pool; deterministic collection. | all above, `rayon` |
| `error` | Operational `Error` type. | `thiserror` |
| `bin/framecensus` | Argument parsing, human renderer, JSON writer, exit codes. | `clap`, library |

Boundaries:

- Only `discover` and `inspect` touch the filesystem.
- `inspect` knows nothing about expectations except the format and required
  channels; it returns a `FileSummary` plus structural findings for one file.
- `consistency` works on summaries only and performs no I/O.
- The human renderer lives in the binary; the library exposes data, not
  formatting, apart from `Finding::message`.

---

## 6. Data flow and lifecycle

```text
 pattern + frames + views ──► Expectation::build ──► validated Expectation
                                                         │
                                         ┌───────────────┘
                                         ▼
                              discover::plan (one directory per view,
                              or one shared directory)
                                         │
                                         ▼
                              discover::list  read_dir once per directory,
                              match names ──► found: BTreeMap<Key, Vec<Entry>>
                                         │
              ┌──────────────────────────┼──────────────────────────────┐
              ▼                          ▼                              ▼
     missing / directory_missing   unexpected / padding / duplicate   present entries
                                                                        │
                                                                        ▼
                                                scan: bounded pool, one task per entry
                                                inspect::file ──► (FileSummary, findings)
                                                                        │
                                                                        ▼
                                                consistency over summaries
                                                                        │
              └──────────────────────────┬──────────────────────────────┘
                                         ▼
                              report::assemble  severity policy, sort,
                              summary counts, outcome
                                         │
                                         ▼
                              CLI: render human | JSON, exit code
```

Lifecycle details:

1. **Validate.** Parse and validate inputs. All configuration errors surface
   here as `Error`, before any I/O.
2. **Plan.** Substitute each view into the directory part of the pattern.
   Several views may share a directory.
3. **Discover.** `read_dir` each distinct directory exactly once. Each entry
   name is matched against the final-component template for every view that
   maps to that directory. Names that are not valid UTF-8 cannot match a
   pattern and are counted in `Summary::skipped_non_utf8`. Non-matching names
   are ignored and counted.
4. **Classify keys.** Walk found keys in order against the frame set:
   expected and found → present; expected and not found → `missing`; found and
   not expected → `unexpected`; more than one entry → `duplicate` (all entries
   are still inspected).
5. **Inspect.** Each present entry becomes one task. A task stats the path,
   rejects non-regular files, opens the file, reads only what the level
   requires, and returns a summary and findings. Results are written into a
   pre-sized slot vector indexed by task number, so collection order is
   independent of completion order.
6. **Compare.** Group summaries by each consistency field. Frames lacking a
   readable header are excluded from grouping and already carry a finding.
7. **Assemble.** Apply the policy, sort, count, compute the outcome.
8. **Render.** The CLI writes the report and exits with the mapped code.

A check is a single synchronous call. Cancellation and progress callbacks are
deferred (section 15).

---

## 7. Algorithms and implementation choices

### 7.1 Frame-set parsing

Grammar (a subset of the [fileseq](https://github.com/justinfx/fileseq)
frame-range syntax):

```text
set     = item ("," item)*
item    = frame | frame "-" frame [ "x" step ]
frame   = ["-"] digits
step    = positive digits
```

Whitespace around items is ignored. `A-B` with `A > B` is rejected (fileseq
treats it as descending; order is irrelevant for a set, so rejecting avoids
silent surprises). Unsupported fileseq forms (`y` inverse step, `:`
staggered) are rejected with an error that names them.

Normalisation inserts each interval into a sorted vector and merges overlaps
and adjacencies, O(n log n) in the number of items.

### 7.2 Pattern tokens and padding

| Token | Meaning |
|---|---|
| `#` run of length *n* | frame, padding *n* (each `#` is one digit) |
| `@` run of length *n* | frame, padding *n* |
| `%0Nd` / `%d` | frame, padding N / 1 |
| `%V` / `%v` | view name / first letter of view name |
| `%%` | literal `%` |

The one-digit-per-`#` rule matches common renderer output conventions (for
example Blender and Nuke). fileseq and OpenImageIO treat a single `#` as four
digits; see open question Q4.

Formatting a frame with padding *p*: the absolute value zero-padded to *p*
digits, then a `-` prefix for negative frames (padding counts digits only;
conventions differ, see Q10).

Matching a file name against the final component: match the leading literal,
read an optional `-` and a maximal ASCII digit run, then match the trailing
literal exactly. The digit run of length *d* is:

- **valid** if `d == p`, or `d > p` and the first digit is not `0` (frame
  number wider than its padding, e.g. `10000` with `####`);
- **padding_mismatch** otherwise (e.g. `01001` or `101` with `####`).

Matching uses no regex engine; it is a linear scan of segment literals.

### 7.3 Expected-set classification

`found` is a `BTreeMap<ExpectedKey, Vec<Entry>>`. For each view, missing
frames are the set difference `FrameSet − found_frames(view)`, computed by a
merge walk over intervals and sorted found frames: O(intervals + found). The
expected set is never enumerated except to emit `missing` findings, which are
bounded by `max_expected_keys`.

If a view's directory does not exist, discovery emits `directory_missing` for
that view and suppresses its per-frame `missing` findings; the summary still
counts the frames as missing.

### 7.4 EXR structural check

Reference: [OpenEXR file layout](https://openexr.com/en/latest/OpenEXRFileLayout.html).

1. **Magic and version.** Read 8 bytes: magic `0x762f3101` and the version
   field (short files: see section 7.9). Version must be 2; flag bits 9 (single-part tiled), 10 (long
   names), 11 (non-image / deep), 12 (multipart) are recorded. Unknown flag
   bits → `unsupported`.
2. **Header bytes.** Read the header into memory with a hard cap
   (`Limits::max_header_bytes`, default 16 MiB, enough for large preview
   attributes). A bounded in-house walker steps over attributes
   (`name\0 type\0 size:i32 value[size]`) and header terminators to find the
   exact byte offset where headers end. It validates every `size` against the
   remaining bytes before skipping. Reaching end of file inside the header →
   `truncated`.
3. **Typed metadata.** The same header bytes are parsed with
   `exr::meta::MetaData::read_from_buffered` from an in-memory slice to obtain
   channels, windows, compression, blocks (scanline or tile description),
   line order, part names, `view` attributes and `chunk_count`. `exr` errors of
   kind `NotSupported` → `unsupported`; `Invalid` → `malformed`.
4. **Offset tables.** For each part, `chunk_count` little-endian `u64` offsets
   follow the headers. Before reading, check
   `header_end + 8 × Σ chunk_count <= file_len`; otherwise `truncated` (this
   also prevents allocating for absurd chunk counts). Read the tables.
5. **Offset validation.** Each offset must be non-zero
   (`offset_table_zero_entry`: OpenEXR writers fill the table on close, so
   zero entries usually mean the writer did not finish), at least
   `table_end` and less than `file_len` (`offset_out_of_range`).
6. **Chunk prefixes.** For each offset, read the fixed-size chunk prefix:
   multipart files start with an `i32` part number; scanline chunks have
   `y: i32` and `data_size: i32`; tiled chunks have four `i32` tile and level
   coordinates and `data_size: i32`. Check that
   - the part number equals the table's part;
   - the coordinates equal the coordinates implied by the table index (the
     table is ordered by block index regardless of line order);
   - `data_size > 0` and `offset + prefix_len + data_size <= file_len`
     (otherwise `truncated`, with the required length as evidence).
7. **Overlap and trailing data.** Sort chunk extents by offset; overlapping
   extents → `malformed` (`chunk_overlap`). Bytes after the maximum chunk end
   → `trailing_data`.
8. **Deep parts** (`deepscanline`, `deeptile`) are summarised from the header,
   and their structure check reports `unsupported` until deep chunk layout is
   implemented (section 15).

Cost per file: one `stat`, one open, one header read, one offset-table read,
and one small positioned read (≤ 28 bytes) per chunk. Pixel data is never read.
Chunk prefix reads use `Seek` + `read_exact` on the task's own file handle
through a small read-ahead buffer, so sequentially stored chunks coalesce
into few system calls.

What this detects: files cut short at any point after the magic number;
offset tables never finalised; tables pointing outside the file or at the
wrong chunk; zero-filled tails (zeroed prefixes fail the coordinate or size
check). What it does not detect: damaged bytes inside a correctly sized chunk.
Open question Q1 covers whether the full chunk walk should be the default on
slow network filesystems.

### 7.5 PNG structural check

Reference: [PNG specification, third edition](https://www.w3.org/TR/png-3/).

1. 8-byte signature, otherwise `wrong_format` (short files: see section 7.9).
2. First chunk must be `IHDR` with length 13; width and height in
   1..=2³¹−1; valid bit-depth and colour-type combination; known compression,
   filter and interlace methods. Violations → `malformed`.
3. Walk chunks: read `length: u32` and `type: [u8; 4]`, validate length
   ≤ 2³¹−1 and type bytes are ASCII letters, then seek over data and CRC.
   A chunk extending past end of file → `truncated`.
4. At least one `IDAT`; all `IDAT` chunks contiguous; `IEND` present with
   length 0. Missing `IEND` at end of file → `truncated`.
5. Bytes after `IEND` → `trailing_data`.

CRCs are not verified at `structure` level because that requires reading
every byte; the deferred `decode` level verifies CRCs and the zlib stream
using the `png` crate.

### 7.6 Required channels

For EXR, required channel names are matched exactly (case-sensitive, full
layer-qualified names such as `diffuse.R`) against the union of channels of
all non-deep parts in the file. Each absent name produces one
`missing_channel` finding. Matching channels inside a specific part or view is
deferred with in-file multiview (section 15).

### 7.7 Consistency

Fields (`Field`, non-exhaustive): `DisplayWindow`, `DataWindow`, `Channels`
(names, pixel types, sampling), `PartLayout` (part count, names, types),
`Compression`, `LineOrder`, `Tiles`, `PngSize`, `PngPixelFormat` (bit depth,
colour type).

Default fields: EXR — `DisplayWindow`, `DataWindow`, `Channels`, `PartLayout`,
`Compression`; PNG — `PngSize`, `PngPixelFormat`.

Algorithm per field:

1. Extract the canonical value from each summary that has one.
2. If a fixed value is declared, every differing frame is `inconsistent`
   with basis `Declared`.
3. Otherwise build `HashMap<value, Vec<task index>>`. The reference group is
   the largest; ties go to the group containing the lowest
   `(view, frame)`. Every frame outside it is `inconsistent` with basis
   `Majority { count, total }`.

Evidence for `inconsistent` includes the reference value and group sizes, so
a report about a mid-sequence resolution change shows both groups even when
the changed stretch is the majority. Comparison is across all views; per-view
grouping is a policy option deferred until a use case requires it.

Renders with automatic data-window cropping legitimately vary `DataWindow`;
users disable it with `--consistent` (Q6).

### 7.8 Parallelism

A dedicated `rayon::ThreadPool` with `jobs` threads runs inspection tasks via
`par_iter` over the present entries. Each task owns its file handle and a
read buffer of at most `max_header_bytes`, so peak memory is roughly
`jobs × max_header_bytes` plus summaries. Discovery and consistency are
single-threaded; they are dominated by directory listing and are linear.

Default `jobs`: `available_parallelism()` clamped to 4–32. Inspection is
I/O-latency bound, so more threads than cores helps on network filesystems
(Q2).

### 7.9 Files shorter than the signature

Format detection compares the first bytes with the expected signature (EXR:
4-byte magic plus 4-byte version field; PNG: 8-byte signature). For a
non-empty file of length `n` shorter than that:

- if its `n` bytes equal the first `n` bytes of the signature, the finding is
  `truncated` (a valid file cut short), with `required_len` set to the
  signature length;
- otherwise the finding is `wrong_format`.

Zero-byte files are always `empty`. A file at least as long as the signature
whose signature bytes differ is `wrong_format`. This makes the truncation
property in section 13 hold for every prefix length, including those shorter
than the signature.

---

## 8. File formats and compatibility

### Input formats

| Format | Version | Initial support | Notes |
|---|---|---|---|
| OpenEXR | file format version 2 | single-part and multipart; scanline and tiled (one level, mipmap, ripmap); all compression codes known to `exr` 1.74 including DWAA/DWAB and HTJ2K | deep parts: header only. Compression only affects lines per chunk at `structure` level, so no decompression support is needed for structural checks. |
| PNG | W3C PNG third edition | static PNG; APNG chunks are walked like any other chunk | CRC and zlib verification at `decode` level only. |

Unknown compression codes in an EXR header are reported as `unsupported`
(the chunk count cannot be derived safely), not as `malformed`.

### Frame-range syntax

Compatible subset of fileseq notation (section 7.1). Canonical output uses
only forms that fileseq also accepts, so ranges can be pasted between tools;
a round-trip test against fileseq's documented examples guards this.

### Report schema

JSON, UTF-8, versioned by a top-level `"schema": "framecensus-report"` and
`"schema_version": 1`.

```json
{
  "schema": "framecensus-report",
  "schema_version": 1,
  "tool_version": "0.1.0",
  "level": "structure",
  "expectation": {
    "pattern": "renders/%V/beauty.####.exr",
    "frames": "1001-1240",
    "views": ["left", "right"],
    "format": "exr",
    "required_channels": ["R", "G", "B", "A", "Z"]
  },
  "summary": {
    "expected": 480, "present": 479, "missing": 1, "unexpected": 1,
    "errors": 11, "warnings": 1, "skipped_non_utf8": 0, "ignored_entries": 3
  },
  "outcome": "fail",
  "findings": [
    {
      "kind": "truncated", "severity": "error",
      "view": "left", "frame": 1010,
      "path": "renders/left/beauty.1010.exr",
      "evidence": { "file_len": 6291456, "required_len": 9437184,
                    "part": 0, "chunk": 61, "chunk_count": 68 },
      "message": "chunk 61 of 68 extends to byte 9437184; file is 6291456 bytes"
    }
  ]
}
```

Compatibility rules:

- Adding optional fields, finding kinds, malformed reasons or `Field` values
  is a minor change; consumers must ignore unknown fields and treat unknown
  kinds by their `severity`.
- Renaming or removing a field or kind, or changing a meaning, increments
  `schema_version`.
- `kind`, `reason` and `field` strings are stable snake_case identifiers.
- Paths are written as given in the pattern (relative or absolute), with `/`
  separators on every platform. Non-UTF-8 paths never appear because they
  cannot match a pattern.
- Exit codes 0/1/2 are stable from the first release.

### Platform compatibility

- File-name matching is byte-exact and case-sensitive on every platform
  (Q9).
- Paths in patterns accept `/` on all platforms and `\` on Windows.
- No reliance on file locks, inotify, or modification times in the initial
  slice.

---

## 9. Errors, findings and reporting

Two channels, never mixed:

**`Error`** (`thiserror`, non-exhaustive) — the check could not run as
specified:

- `InvalidPattern { pattern, position, reason }`
- `InvalidFrameSet { input, position, reason }`
- `InvalidExpectation { reason }` (views without view token, PNG with
  required channels, empty frame set, invalid view name)
- `LimitExceeded { limit, value }`
- `ThreadPool(..)`

Every `Error` maps to CLI exit code 2 and a single-line message on stderr
that points at the offending input position.

**Findings** — anything about the scanned files or directories, including I/O
errors on individual files (`unreadable` with the `io::ErrorKind` and OS
message as evidence) and a missing target directory. A run that finds every
file broken still returns `Ok(Report)`.

Rules:

- Every finding has evidence specific enough to act on: byte offsets and
  lengths for structural findings, expected and actual values for
  consistency, the expected path for `missing`.
- One root cause, one finding per file: a truncated header yields
  `truncated`, not an additional `missing_channel` for every channel. Checks
  that depend on a failed stage are skipped and the summary records the level
  each file reached.
- `Finding::message` is a derived English sentence; tools should rely on
  `kind` and `evidence`.
- Severity comes only from the `Policy`, so CI can tighten (`--deny
  unexpected`) or relax (`--allow unsupported`) without code changes.

---

## 10. Performance and resources

Targets to verify during implementation (not measured yet):

- `structure` level on 100 000 typical multichannel scanline EXRs on a local
  SSD completes in tens of seconds, dominated by `open` and positioned reads.
- Bytes read per EXR ≈ header size + 8 × chunks + ≤ 28 × chunks, independent
  of pixel data size.
- Memory for 1 000 000 present files stays within a few hundred MiB
  (summaries ~100–200 bytes each with interned channel lists).

Cost model: O(D) directory entries for discovery, O(E) for classification with
E bounded by found files plus emitted findings, and O(Σ chunks) small reads
for inspection.

Network filesystems: latency per round trip dominates. Mitigations built in
from the start: one `read_dir` per directory, no per-frame `stat` for missing
frames (absence is inferred from the listing), a read-ahead buffer that
coalesces adjacent chunk prefixes, and configurable `jobs`.

Limits (`Limits`, all overridable):

| Limit | Default | Purpose |
|---|---|---|
| `max_expected_keys` | 10 000 000 | rejects mistyped ranges before allocation |
| `max_header_bytes` | 16 MiB | caps per-file header memory |
| `max_chunks_per_file` | 16 777 216 | caps offset-table memory (also bounded by file length / 8) |
| `max_directory_entries` | 10 000 000 | caps discovery memory |

Benchmarks: `criterion` microbenchmarks for pattern matching and frame-set
operations; an ignored-by-default integration benchmark that generates a
synthetic sequence and times each level.

---

## 11. Security and robustness

Threat model: scanned files and file names may be arbitrary bytes (corrupt
renders, unrelated files, hostile files in shared storage). Expectations come
from the operator but may be generated by scripts.

- **No unsafe code** (`#![forbid(unsafe_code)]`). Positioned reads use
  `Seek`; no memory mapping, so a file truncated during the scan cannot crash
  the process.
- **Bounded allocation.** Every length from a file is validated against the
  remaining file length and against `Limits` before allocation or skipping.
  Header bytes are copied into a capped buffer before `exr` parses them, so the
  dependency cannot read beyond the cap.
- **No panics on input.** Arithmetic on file-derived values uses checked
  operations. Fuzz targets cover our parsers. As containment for dependency
  bugs, each inspection task runs inside `std::panic::catch_unwind`; a caught
  panic becomes a `malformed` finding with reason `internal_error`, and the
  run continues. Containment does not replace fuzzing.
- **Non-regular files.** Files are opened only after metadata shows a regular
  file, so FIFOs and devices are not opened in normal operation (opening a
  FIFO for reading blocks). To close the race where a path is replaced
  between `stat` and `open`, Unix builds open with `O_NONBLOCK` through the
  safe `OpenOptionsExt::custom_flags` and re-check the handle's metadata
  before the first read; this is the only platform-specific code.
- **Symlinks** are followed by default (render outputs are often linked);
  `--no-follow-symlinks` reports links as `not_a_file`. Dangling links are
  `not_a_file` either way.
- **Path construction.** View names cannot contain separators or `..`, so
  substituting them cannot escape the pattern's directories. FrameCensus
  never deletes, writes or renames scanned files; the only write is the report
  file named by `--output`.
- **No network, no subprocesses, no environment-dependent behaviour.**
- **Terminal output** escapes control characters in file names before
  printing.

---

## 12. Dependencies and alternatives

| Crate | Use | Licence | Alternatives considered |
|---|---|---|---|
| [`exr`](https://crates.io/crates/exr) 1.74 | Typed EXR header metadata, chunk counts, later pixel decode | BSD-3-Clause | Own typed header parser (more code to maintain); [`openexr`](https://crates.io/crates/openexr) bindings to the C++ library (C++ toolchain, harder cross-platform builds); OpenImageIO bindings (very large dependency). |
| [`png`](https://crates.io/crates/png) 0.18 | `decode` level only (CRC, zlib) | MIT OR Apache-2.0 | `image` (much broader than needed). |
| [`rayon`](https://crates.io/crates/rayon) | Bounded worker pool | MIT OR Apache-2.0 | `std::thread::scope` + channel (fewer dependencies, more code); async runtime (no benefit for blocking file I/O). |
| [`serde`](https://crates.io/crates/serde), [`serde_json`](https://crates.io/crates/serde_json) | Report serialization | MIT OR Apache-2.0 | none reasonable |
| [`thiserror`](https://crates.io/crates/thiserror) | Error type | MIT OR Apache-2.0 | hand-written `impl Error` |
| [`libc`](https://crates.io/crates/libc), Unix only | `O_NONBLOCK` constant for safe `custom_flags` | MIT OR Apache-2.0 | `rustix` (broader); hard-coded constants (per-platform values) |
| [`clap`](https://crates.io/crates/clap) (derive), `cli` feature only | CLI parsing | MIT OR Apache-2.0 | `lexopt` (smaller, more code) |

Dev dependencies: `proptest`, `insta` (snapshot tests of JSON and human
output), `tempfile`, `assert_cmd`, `criterion`. Fuzzing: `cargo-fuzz` with
`libfuzzer-sys` in a separate `fuzz/` crate outside the published package.

Not used: regex (pattern matching is a linear scan), glob crates (the pattern
is not a glob), memory-mapping crates (unsafe and fragile with truncation),
an async runtime.

`cargo-deny` configuration allows MIT, Apache-2.0, BSD-2/3-Clause, ISC,
Unicode-3.0 and Zlib, denies unknown registries and git sources, and fails on
RustSec advisories.

MSRV 1.85, the minimum for edition 2024. The runtime dependencies fit it
(`clap` 4.6 and `assert_cmd` 2.2 need 1.85, `exr` 1.74 needs 1.83). Edition
2024 uses the MSRV-aware resolver, so dev dependencies resolve to versions
that support 1.85 (for example a `proptest` release older than 1.12, which
needs 1.88). CI runs `cargo +1.85 test` alongside stable.

---

## 13. Testing strategy

### Unit tests

- `frameset`: parsing of every grammar form, rejection of unsupported fileseq
  forms with positions, normalisation, `contains`, `len`, formatting.
- `pattern`: token parsing, `%%`, adjacency rejection, padding rule table
  (exact, wider, leading zero, shorter, negative), view substitution.
- `inspect::exr`: attribute walker on hand-built headers; chunk-count and
  expected-coordinate computation per block layout; each malformed reason.
- `inspect::png`: each chunk-order rule.
- `consistency`: majority, ties, declared values, absent values.
- `report`: ordering, policy overrides, outcome.

### Property tests (`proptest`)

- `FrameSet::from_str(set.to_string()) == set`; set operations agree with a
  `BTreeSet<i64>` oracle on small domains.
- For random patterns, views, frames and paddings, `match(format(key)) ==
  Some(key)`, and names with wrong padding are classified `padding_mismatch`.
- **Truncation property:** for every generated valid EXR and PNG fixture and
  every strict prefix length `0 < n < len`, `structure` level reports
  `truncated` or `malformed` (prefixes shorter than the signature per
  section 7.9), never a clean file. Fixtures are small so all
  prefixes are tested; larger fixtures use sampled lengths.
- **Zero-fill property:** replacing any suffix of a valid file with zeros
  while keeping the length is never reported clean, except where the zeroed
  bytes lie entirely inside chunk data (the documented limit; the test
  asserts that boundary explicitly).
- Inspection never panics on arbitrary byte strings.
- Determinism: `check` with `jobs = 1` and `jobs = 16` yields identical JSON.

### Fixture generation

A `tests/support` module builds sequences in temporary directories:

- valid EXRs written with `exr` (scanline, tiled, mipmap, multipart; several
  compressions) and PNGs written with `png`;
- byte-level mutations: truncate at offset, zero tail, zero offset-table
  entry, corrupt chunk coordinate, append trailing bytes, change magic;
- header variants: different resolution, channel set, pixel type,
  compression.

A small set of committed fixture files (a few KiB each) covers layouts the
`exr` crate cannot write, such as deep parts and files from other writers, with
their provenance and licence recorded next to them.

### Integration tests

- The reference scenario: a 20-frame expected range with a missing first
  frame, a truncated frame, a zero-byte frame, a resolution change, a channel
  change and missing last frames. The test asserts the exact finding list and
  exit code 1.
- CLI tests with `assert_cmd`: exit codes 0/1/2, JSON snapshots with `insta`,
  human-output snapshots, `--deny` / `--allow` behaviour, invalid inputs.
- Filesystem cases: missing directory, unreadable file (Unix permission),
  FIFO (Unix), dangling symlink, directory named like a frame.

### Differential tests (optional CI job)

A container job with OpenEXR command-line tools runs `exrcheck` on the
generated corrupt corpus and records agreement with FrameCensus `structure`
verdicts. Disagreements on chunk-data corruption are expected and listed;
disagreements on truncation fail the job.

### CI matrix

Linux, macOS and Windows on stable; Linux on MSRV; `fmt`, `clippy -D
warnings`, `doc` with `-D warnings`, `cargo deny check`; fuzz targets run
briefly on a schedule.

---

## 14. Initial usable slice

The first release is usable as a farm post-job and CI check for single-file-
per-view EXR or PNG sequences.

Included:

1. `frameset` and `pattern` with the grammar and tokens in section 7.
2. `Expectation` builder with views, format, required channels, fixed display
   and data windows, consistency fields, minimum size.
3. Discovery and all expected-set findings, including `directory_missing`.
4. Levels `exists`, `header`, `structure`.
5. EXR: single-part and multipart, scanline and tiled, all compression codes
   known to `exr`; deep parts → `unsupported`.
6. PNG: signature, IHDR, chunk walk, IEND, trailing data.
7. Consistency with declared and majority basis.
8. Severity policy, outcome, JSON schema v1, human table with range
   collapsing, exit codes.
9. `framecensus check` and `framecensus inspect`.
10. Bounded parallel inspection with deterministic output.
11. Unit, property, integration and fuzz tests as in section 13; CI as listed.

Acceptance for the slice: the reference scenario reports exactly the
expected findings at `structure` level; the truncation property holds for all
generated fixtures; JSON is identical for `jobs = 1` and `jobs = 16`; no
`clippy` warnings; documentation on every public item.

---

## 15. Deferred functionality and non-goals

### Deferred (designed for, not in the first slice)

- **`decode` level**: full pixel decoding with `exr` and `png` to detect
  damaged compressed data and PNG CRC errors; optional constant-image
  detection as an informational finding.
- **In-file multiview EXR**: expectations for views stored as parts with a
  `view` attribute or via the single-part `multiView` attribute and
  channel-name prefixes ([multi-view OpenEXR](https://openexr.com/en/latest/MultiViewOpenEXR.html)).
- **Deep EXR structure**: deep scanline and deep tile chunk layouts.
- **Additional formats**: DPX and TIFF header and structure checks.
- **Expectation file**: a TOML file describing several sequences checked in
  one run, mapping one-to-one to `Expectation` fields.
- **Settle-time policy** for files that may still be written: skip or warn on
  files modified within a window.
- **Watch and incremental modes** for long-running farm jobs.
- **JUnit XML and SARIF** report formats.
- **Additional pattern tokens**: Houdini-style `$F4`, fileseq `y` and `:`
  range forms, frame tokens in directory names.
- **Library cancellation and progress callbacks.**
- **Relative size outliers** (frames much smaller than their neighbours) as a
  warning heuristic.

### Non-goals

- Visual regression and image-difference testing.
- Temporal analysis (flicker, drift, motion).
- Render scheduling, job submission, re-queueing, farm management.
- Repairing, rewriting or converting files.
- Discovering sequences without an expectation (existing tools do this well).
- Object storage or remote protocols; FrameCensus reads a mounted filesystem.

---

## 16. Open technical questions

Each question has a recommended default that implementation can start with.

| # | Question | Recommended default |
|---|---|---|
| Q1 | Is the full chunk-prefix walk cheap enough on network filesystems for six-figure frame counts, or is a cheaper "table only" structure depth needed (offset table range checks plus only the chunk with the highest offset)? | Full walk by default. Add a `table` depth only if benchmarks on NFS show the walk dominates; it still detects end-of-file truncation. |
| Q2 | Default worker count for network filesystems. | `available_parallelism()` clamped to 4–32; document `--jobs` for NFS tuning; revisit after benchmarks. |
| Q3 | Does `exr` parse every real-world header FrameCensus will see (long names, unusual attributes, deep parts, very new compression codes), and does it expose the header end offset? | Use `exr` for typed metadata on an in-memory copy of the header; use the in-house attribute walker for byte offsets. Collect real-world headers in the fixture set; if `exr` rejects valid headers, report `unsupported` and consider a minimal typed parser for the few attributes needed. |
| Q4 | `#` padding convention: one digit per `#` (Blender, Nuke) or `#` = four digits (fileseq, OpenImageIO)? | One digit per `#`. Recommend `%04d` in documentation as unambiguous. Add `--hash-style` only on demand. |
| Q5 | How to treat files still being written when a check runs before a job finishes. | No settle logic initially; checks are expected to run after the job ends. A settle-time option is deferred. |
| Q6 | Should `DataWindow` be compared by default, given renderers with automatic cropping? | Yes, compared by default; documented opt-out. A restarted task with a new resolution is the more common and costly failure. |
| Q7 | Reference group for consistency when no value is declared. | Largest group; tie broken by earliest `(view, frame)`. Evidence always shows group sizes. |
| Q8 | Severity of `unsupported`. | Warning (exit 0) so new features do not break existing checks; `--deny unsupported` for strict pipelines. |
| Q9 | Case-insensitive filesystems: should `Beauty.1001.EXR` match `beauty.####.exr`? | No; byte-exact matching on all platforms for deterministic results. Revisit with a `case_mismatch` warning if users hit it. |
| Q10 | Negative frame padding: does padding count the sign (`printf` `%04d` gives `-001`) or digits only (`-0001`)? Tools disagree. | Digits only, applied identically to `#`, `@` and `%0Nd`. Document it, and verify against the conventions of the renderers users report before the first release. |
| Q11 | One package with a `cli` feature, or separate library and CLI crates? | One package, `cli` as a default feature with `required-features` on the binary. Split later only if the CLI grows its own release cadence. |
| Q12 | Should `structure` verify PNG CRCs, which requires reading the whole file? | No; CRCs belong to `decode`. PNG frames are usually small, so revisit if users ask for it. |
| Q13 | Are per-frame JSON findings too large when an entire sequence is missing? | Keep one finding per frame for actionability (bounded by `max_expected_keys`); `directory_missing` already collapses the common case. Add a compact `missing_frames` range string to the summary. |
