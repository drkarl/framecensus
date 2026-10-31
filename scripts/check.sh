#!/usr/bin/env bash
# Single quality gate: run locally and by CI. Fails on the first failing step.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

step() { printf '\n==> %s\n' "$*"; }

if ! cargo deny --version 2>/dev/null | grep -q '0\.19'; then
  echo "error: cargo-deny 0.19.x is required (missing or wrong version)." >&2
  echo "install it with: cargo install cargo-deny --version 0.19.0 --locked" >&2
  exit 1
fi

step "cargo fmt --check"
cargo fmt --check

step "cargo clippy --all-targets -- -D warnings"
cargo clippy --locked --all-targets -- -D warnings

step "cargo test"
cargo test --locked

step "cargo doc --no-deps (warnings denied)"
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps

step "cargo deny check"
cargo deny check

printf '\nAll checks passed.\n'
