//! Structural completeness checks for rendered image sequences.
//!
//! FrameCensus reports frames that are missing, unexpected, empty, unreadable,
//! truncated, malformed or inconsistent with the rest of a sequence.
//!
//! This package is a scaffold: the public API and the checks are not
//! implemented yet. It exists so later work builds on a package that already
//! enforces the project's quality rules.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
