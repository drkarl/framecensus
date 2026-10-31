# FrameCensus roadmap

Status: **proposed.** This roadmap turns the [architecture](architecture.md)
into milestones and tasks. It does not change the design; section references
such as "§7.4" point into the architecture. Nothing here is implemented yet.

## Contents

1. [Summary](#1-summary)
2. [How the estimates were made](#2-how-the-estimates-were-made)
3. [Quality gates](#3-quality-gates)
4. [Milestones](#4-milestones)
5. [Implementation order and dependencies](#5-implementation-order-and-dependencies)
6. [Risks](#6-risks)
7. [Architecture questions and where they are settled](#7-architecture-questions-and-where-they-are-settled)
8. [Deferred work](#8-deferred-work)
9. [Notes on the architecture document](#9-notes-on-the-architecture-document)

---

## 1. Summary

The architecture's "initial usable slice" (§14) is the complete first release.
This roadmap splits it into five milestones so that something useful exists
early: M1 already answers the question existing sequence tools cannot ("which
frames of the declared range are missing, unexpected or empty?"), and each
later milestone adds a deeper level of evidence.

| Milestone | Outcome | Tasks | Estimate (engineer-hours) | Cumulative |
|---|---|---|---|---|
| **M1** Sequence census | `framecensus check` validates a numbered sequence against a declared range and views at the `exists` level, with human and JSON output and CI exit codes. | 9 | 73–111 | 73–111 |
| **M2** Inspection engine and PNG | Parallel, bounded file inspection; PNG sequences are checked at `header` and `structure` level. | 4 | 44–65 | 117–176 |
| **M3** OpenEXR structure | EXR sequences (single-part, multipart, scanline, tiled) are checked at `header` and `structure` level. | 5 | 54–84 | 171–260 |
| **M4** Consistency, channels, inspect | Required channels, cross-frame consistency, severity overrides, `inspect` command. Feature-complete for the architecture's first release. | 5 | 40–60 | 211–320 |
| **M5** Hardening and 0.1.0 | Fuzzing, benchmarks, optional differential job, documentation, release preparation. | 4 | 40–66 | 251–386 |

**Total: about 250–390 engineer-hours (midpoint roughly 320) for 27 tasks.**
The estimate is for one engineer who is comfortable with Rust and file
formats, includes tests and documentation for each task, and excludes waiting
time for review.

The numbered tasks below correspond one-to-one to the issue drafts prepared
for publication. Task numbers are stable within this document; they will map
to repository issue numbers once published.

## 2. How the estimates were made

- Each task is estimated as a range: the low end assumes the design holds and
  no surprises; the high end assumes one round of rework or one dependency
  surprise.
- Ranges are per task and are summed per milestone, so a milestone range is
  wider than its tasks' typical outcome. The midpoint is the better planning
  figure; the low end is not a commitment.
- "Done" always includes unit and integration tests, documentation of every
  public item, and passing the common quality gates.
- Not included: reviewing and merging time, publishing to a registry, support after release, and any item in
  [deferred work](#8-deferred-work).
- Estimates will be re-based after M1 using the actual hours spent; the
  largest uncertainty is EXR structure (M3), which is the least familiar
  territory.

## 3. Quality gates

### Common gates (every task, every milestone)

| Gate | Command or check |
|---|---|
| Format | `cargo fmt --all --check` |
| Lints | `cargo clippy --workspace --all-targets -- -D warnings`; library code denies `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic` (§2 N7) |
| Tests | `cargo test --workspace` on Linux, macOS and Windows (stable) and Linux on the declared MSRV 1.85 |
| Docs | `cargo doc --no-deps` with `RUSTDOCFLAGS="-D warnings"`; `#![warn(missing_docs)]` clean |
| Supply chain | `cargo deny check` (licences, advisories, sources, duplicate versions) |
| Safety | `#![forbid(unsafe_code)]`; no new `unwrap`/`expect`/`panic!` in library code |
| Hygiene | public API and JSON changes are listed in the pull request description; fixtures carry provenance and licence notes |

### Per-milestone gates (in addition)

| Milestone | Additional gates |
|---|---|
| M1 | Reference scenario (missing first frame, empty frame, unexpected frame, padding mismatch, absent directory) yields the exact finding list and exit code 1 at `exists` level. `FrameSet` and `FramePattern` property tests pass. JSON output validates against the committed schema file. |
| M2 | `jobs = 1` and `jobs = 16` produce byte-identical JSON. The truncation property holds for every generated PNG fixture and every strict prefix. A task that panics in a fixture is reported as `malformed` (`internal_error`) and the run continues. |
| M3 | The truncation property holds for every generated EXR fixture (scanline, tiled, mipmap, multipart). The zero-fill property holds except inside chunk data, and the test asserts that boundary. Hand-built malformed headers never panic. |
| M4 | The full reference scenario from §13 (missing first frame, truncated, zero-byte, resolution change, channel change, missing last frames) reports exactly the expected findings. README examples run as documented, verified by a CLI test. |
| M5 | Fuzz targets run for a fixed budget with no crashes. Benchmarks are recorded and the §10 targets are confirmed or revised. Release checklist complete (see Task 27). |

## 4. Milestones

Tasks are listed in implementation order. "Depends on" lists task numbers that
must be merged first. Estimates are engineer-hours.

### M1: Sequence census (`exists` level)

**Goal.** A usable first slice: declare a pattern, frame set and optional
views; get every missing, unexpected, duplicate, mis-padded, empty,
undersized and non-regular file, plus a missing-directory finding, as a table
or JSON, with stable exit codes. No file content is read. This is already
enough to catch missing leading/trailing frames and stale frames from an
earlier render, which discovery-based tools cannot report.

**Scope boundary.** No format parsing, no worker pool, no consistency.

| # | Task | Depends on | Estimate |
|---|---|---|---|
| 01 | Package scaffold, lint configuration and CI | — | 6–10 |
| 02 | Frame sets: parsing, normalisation, formatting | 01 | 8–12 |
| 03 | Frame patterns: tokens, formatting, name matching, padding rule | 02 | 10–14 |
| 04 | Report model, severity policy and JSON schema v1 | 01 | 10–15 |
| 05 | Expectation builder, limits and operational errors | 02, 03 | 6–9 |
| 06 | Directory discovery and expected-set findings | 04, 05 | 12–18 |
| 07 | File-level checks and the `check` entry point at `exists` level | 06 | 6–10 |
| 08 | `framecensus check` command: human table, JSON, exit codes | 07 | 10–15 |
| 09 | M1 integration tests and README alignment | 08 | 5–8 |

**M1 total: 73–111 h.**

**Exit criteria.** The M1 gates pass; `framecensus check` runs against a
real directory tree on all three platforms; the human and JSON output for the
reference scenario are snapshot-tested; README text matches actual behaviour
(proposal labels removed only for what exists).

**Tests specific to M1.** Property tests for `FrameSet` round-trips and set
operations against a `BTreeSet<i64>` oracle; pattern format/match round-trips
and padding classification; filesystem cases (missing directory, directory
named like a frame, dangling symlink, FIFO on Unix, unreadable directory,
non-UTF-8 name on Unix); CLI exit codes 0/1/2; snapshot tests of JSON and
human output.

### M2: Inspection engine and PNG

**Goal.** The bounded, deterministic machinery that every deeper check needs,
proven on the simpler format. PNG sequences gain `header` and `structure`
checks, so a truncated PNG frame is now reported as `truncated`.

| # | Task | Depends on | Estimate |
|---|---|---|---|
| 10 | Test support: sequence builder, byte mutations and prefix/zero-fill property harness | 01 | 8–12 |
| 11 | Inspection framework: safe open, bounded reader, format detection, short-file rule, panic containment | 07 | 12–18 |
| 12 | Parallel scan, deterministic collection, `--level` and `--jobs` | 08, 11 | 10–15 |
| 13 | PNG header and structure checks | 10, 11 | 14–20 |

**M2 total: 44–65 h.**

**Exit criteria.** M2 gates pass. `check` on a PNG sequence at each of the
three levels behaves as the §1 table states; the report records the level
used; the truncation property holds for all generated PNG fixtures.

**Tests specific to M2.** Truncation and zero-fill properties over generated
PNGs; every chunk-order rule from §7.5; short-file rule for every prefix
length shorter than the signature; determinism across worker counts; a
panic-injection test at the task boundary (a test-only hook, not shipped
behaviour); non-regular-file handling including the post-`stat` race re-check.

### M3: OpenEXR structure

**Goal.** EXR sequences are checked at `header` and `structure` level without
reading pixel data. This is the milestone with the most uncertainty; it starts
with a short spike to confirm the `exr` crate behaves as the design assumes
(Q3).

| # | Task | Depends on | Estimate |
|---|---|---|---|
| 14 | EXR header walker and typed metadata (includes the Q3 spike) | 10, 11 | 14–22 |
| 15 | EXR offset tables and chunk-prefix validation, single-part scanline | 14 | 14–22 |
| 16 | EXR tiled files: one-level, mipmap and ripmap | 15 | 10–16 |
| 17 | EXR multipart files and deep parts | 15 | 8–12 |
| 18 | EXR overlap, trailing data and exhaustive truncation properties | 16, 17 | 8–12 |

**M3 total: 54–84 h.**

**Exit criteria.** M3 gates pass. Every generated EXR layout is clean when
intact and never clean under any strict prefix; each `MalformedReason` in §4
that applies to EXR has a test that triggers it; deep parts report
`unsupported` with the feature named; `exr` rejections map to `unsupported`
or `malformed` as §7.4 step 3 specifies.

**Spike decision rule (Task 14).** If `exr` cannot parse headers that
OpenEXR writers produce (long names, unusual attributes, deep parts) in
more than a rare case, record the failing headers as fixtures, report
`unsupported` for them, and open a follow-up for a minimal typed header parser
rather than expanding scope inside the task.

### M4: Consistency, channels and `inspect`

**Goal.** The remaining first-release capabilities: required channels,
cross-frame consistency with declared or majority basis, severity overrides
and the single-file `inspect` command. After M4 the architecture's initial
slice (§14) is functionally complete.

| # | Task | Depends on | Estimate |
|---|---|---|---|
| 19 | Required EXR channels and `missing_channel` | 14 | 4–7 |
| 20 | Cross-frame consistency, fixed values and CLI flags | 12, 13, 14 | 14–20 |
| 21 | Severity overrides, report output file and per-file summaries | 04, 08 | 8–12 |
| 22 | `inspect` command and `inspect_file` | 13, 14, 08 | 8–12 |
| 23 | Reference scenario, CLI documentation and README alignment | 19, 20, 21, 22 | 6–9 |

**M4 total: 40–60 h.**

**Exit criteria.** M4 gates pass. The reference scenario from §13 is a
committed integration test with an exact finding list. Tie-breaking and
evidence for `inconsistent` follow Q7. README claims are all backed by tests.

### M5: Hardening and 0.1.0

**Goal.** Confidence that untrusted input cannot crash or stall the tool, that
performance claims are measured, and that the first release is documented and
publishable.

| # | Task | Depends on | Estimate |
|---|---|---|---|
| 24 | Fuzz targets and scheduled fuzz job | 02, 03, 13, 18 | 10–16 |
| 25 | Benchmarks, resource-limit validation and Q1/Q2 decisions | 12, 18 | 12–20 |
| 26 | Differential check against OpenEXR's command-line tools (optional CI job) | 18 | 8–14 |
| 27 | Release readiness: documentation, metadata, licence check, MSRV job | 23, 24, 25 | 10–16 |

**M5 total: 40–66 h.**

**Exit criteria.** M5 gates pass. Measured numbers replace the §10 targets in
the documentation (whether they confirm or revise them). A `0.1.0` tag is
prepared; publishing to a registry is a separate step.

Task 26 is optional: it needs a container image with OpenEXR tools and can be
dropped without blocking the release. If dropped, subtract 8–14 h.

## 5. Implementation order and dependencies

Critical path: 01 → 02 → 03 → 05 → 06 → 07 → 11 → 13 (Task 13 also needs
Task 10) → … Task 12 follows 08 and 11. The main
parallel opportunities, if more than one engineer is available, are:

- Task 04 (report model) alongside Tasks 02–03.
- Task 10 (test support) at any time after Task 01.
- Tasks 14–17 (EXR) alongside Task 13 (PNG) once Task 11 is merged.
- Tasks 19 and 21 are independent of consistency (Task 20).

```text
M1: 01 ─┬─ 02 ─ 03 ─ 05 ─┐
        └─ 04 ───────────┴─ 06 ─ 07 ─ 08 ─ 09
M2: 01 ─ 10 ──────────┐
    07 ─ 11 ──────────┼─ 13        08,11 ─ 12
M3: 10,11 ─ 14 ─ 15 ─┬─ 16 ─┐
                     └─ 17 ─┴─ 18
M4: 14 ─ 19      12,13,14 ─ 20      04,08 ─ 21      13,14,08 ─ 22
    19,20,21,22 ─ 23
M5: 02,03,13,18 ─ 24      12,18 ─ 25      18 ─ 26      23,24,25 ─ 27
```

Single-engineer order: 01, 02, 03, 04, 05, 06, 07, 08, 09, 10, 11, 12, 13,
14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27. Task 10 may move
earlier if property harness work is wanted for M1 tests.

## 6. Risks

| # | Risk | Likelihood | Impact | Mitigation | Affects |
|---|---|---|---|---|---|
| R1 | The `exr` crate rejects or mis-parses some valid headers, or does not expose what the structural walk needs (Q3). | Medium | High | Task 14 starts with a timeboxed spike; header bytes are parsed from an in-memory copy; failing headers become fixtures and `unsupported` findings; minimal in-house typed parser is the fallback and is a separate task if needed. | M3 |
| R2 | EXR chunk-coordinate expectations are wrong for tiled mipmap/ripmap or unusual line orders, producing false `malformed` findings. | Medium | High | Generate fixtures with an independent writer (`exr`) and a few committed files from other writers; run the optional differential job (Task 26); property test that intact generated files are never reported. | M3 |
| R3 | Memory growth when an entire large sequence is missing: one finding per frame, each carrying an expected path, can reach `max_expected_keys` (10 M) findings and exceed the N3 budget. | Medium | Medium | Keep the designed `Evidence::Missing { expected_path: PathBuf }`; measure memory in Task 25; if the measurement shows a problem, propose an explicit architecture amendment (for example storing only the key, a lower default limit, or the compact range summary from Q13). | M1, M5 |
| R4 | Behaviour differs on Windows and macOS (path separators in patterns, case-insensitive filesystems, permission tests, FIFOs). | Medium | Medium | CI matrix from Task 01; Unix-only tests are gated; byte-exact matching is documented (Q9); JSON paths use `/` everywhere. | all |
| R5 | Network filesystem latency makes the full chunk walk too slow (Q1). | Medium | Medium | Task 25 measures on a loopback-mounted network filesystem if available; the cheaper `table` depth is designed but built only on evidence. | M5 |
| R6 | Tests that depend on permissions or special files behave differently when CI runs as root or in containers. | Medium | Low | Detect and skip with an explicit message; keep the logic in unit tests that use injected metadata where possible. | M1, M2 |
| R7 | `catch_unwind` containment is defeated by `panic = "abort"` in a downstream profile or by panics in worker threads outside the task. | Low | Medium | Document the requirement; keep the catch at the task boundary; test with the default profile; fuzzing (Task 24) is the real defence. | M2, M5 |
| R8 | Padding convention choices (Q4, Q10) surprise users of other tools. | Medium | Low | Recommended defaults are adopted; the documentation recommends `%04d`; verification against renderer behaviour happens in Task 27 before the first release. | M1, M5 |
| R9 | Fuzzing finds many robustness bugs late, extending M5. | Medium | Medium | Parsers are written with checked arithmetic and bounded reads from the start (gate in every parser task); early short fuzz runs are encouraged during M2 and M3 even though the formal task is in M5. | M5 |
| R10 | Estimates are low because EXR behaviour has many edge cases. | Medium | Medium | M3 ranges are the widest; re-baseline after Task 15; defer tiled ripmap handling to a follow-up if it exceeds its range by more than 50%. | M3 |

## 7. Architecture questions and where they are settled

The architecture's recommended defaults are adopted. Where a question needs
evidence, the table names the task that produces it.

| Question | Handling |
|---|---|
| Q1 full chunk walk vs `table` depth | Full walk implemented in Task 15; measured in Task 25; `table` depth only if the measurement justifies it. |
| Q2 default worker count | Implemented as designed in Task 12; revisited in Task 25. |
| Q3 `exr` coverage | Spike inside Task 14. |
| Q4 `#` padding | One digit per `#` (Task 03); documented in Task 27. |
| Q5 files still being written | Deferred (§8). |
| Q6 `DataWindow` compared by default | Task 20. |
| Q7 reference group | Task 20. |
| Q8 severity of `unsupported` | Warning by default (Task 04). |
| Q9 case sensitivity | Byte-exact (Task 03). |
| Q10 negative-frame padding | Digits only (Task 03); verified in Task 27. |
| Q11 one package with a `cli` feature | Task 01. |
| Q12 PNG CRC | Not in `structure`; deferred with `decode`. |
| Q13 per-frame missing findings | One per frame; the compact range summary is decided in Task 25 from measurements. |

## 8. Deferred work

These items come from §15 and are **not** part of the committed roadmap. They
have no tasks or estimates yet; each should be estimated when it is chosen,
with the listed trigger.

| Item | Why deferred | Trigger to schedule |
|---|---|---|
| `decode` level (pixel decoding, PNG CRC, constant-image info finding) | Heavy cost per file; the structural levels cover truncation. | Users report damaged compressed data that `structure` misses. |
| In-file multiview EXR | Needs per-part and per-view channel matching. | Stereo pipelines storing both views in one file. |
| Deep EXR structure | Layout work with few test files. | Deep renders in scope for users. |
| DPX and TIFF | New parsers; formats outside the first audience. | Concrete demand with sample files. |
| Expectation file (multiple sequences per run) | CLI and library cover one sequence. | Users scripting many sequences per job. |
| Settle-time policy, watch and incremental modes | Checks are expected to run after the job ends. | Users want checks during a running job. |
| JUnit XML and SARIF output | JSON is sufficient for scripts. | CI integration requests. |
| Additional pattern tokens (`$F4`, `y`/`:` ranges, frame tokens in directories) | Subset covers common cases. | Requests from specific pipelines. |
| Cancellation and progress callbacks | Single synchronous call is enough for the CLI. | Embedding in an interactive tool. |
| Relative size outliers heuristic | Heuristic with false positives. | After real-world data exists. |
| Per-view consistency grouping | Cross-view comparison is the default. | A concrete case where views legitimately differ. |
| `--hash-style` and case-mismatch warning | Defaults are documented. | User reports. |

## 9. Notes on the architecture document

These do not block the roadmap and are recorded so the design owner can decide
whether to amend the architecture:

1. **Stale cross-references.** The CLI listing says the `--consistent` default
   is "see §7.5" (PNG structure); the consistency defaults are in §7.7. The
   `Options::jobs` comment says "see §10"; the default is stated in §7.8.
2. **Per-finding memory.** `Evidence::Missing { expected_path: PathBuf }`
   combined with one `missing` finding per expected key and
   `max_expected_keys = 10 000 000` can exceed the "few hundred MiB" target
   in §10 when a whole large sequence is absent. Risk R3 describes the
   handling: the roadmap keeps the designed type and defers any change to
   Task 25. Changing the evidence variant would be an explicit architecture
   amendment, decided after the Task 25 measurements.
3. **Scope of the "initial slice".** §14 describes the whole first release;
   this roadmap's M1 is a strictly smaller, earlier slice. No design change is
   implied.
