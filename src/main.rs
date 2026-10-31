//! Command-line entry point for FrameCensus.

#![forbid(unsafe_code)]

use clap::Parser;

/// Structural completeness checks for rendered image sequences.
#[derive(Debug, Parser)]
#[command(name = "framecensus", version, about, arg_required_else_help = true)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
