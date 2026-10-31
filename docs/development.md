# Local quality gates

CI runs the same gates. `scripts/check.sh` is the single local entry point for
the format, lint, test, documentation and supply-chain checks.

## Prerequisites

- Rust: `rust-toolchain.toml` selects the pinned stable toolchain with
  `rustfmt` and `clippy`. The declared MSRV is 1.85 (`rust-version` in
  `Cargo.toml`); install it with `rustup toolchain install 1.85.0`.
- `cargo-deny` 0.19:
  `cargo install cargo-deny --version 0.19.0 --locked`.

## Gates

| Gate | Command |
|---|---|
| Format | `cargo fmt --check` |
| Lints | `cargo clippy --all-targets -- -D warnings` |
| Tests | `cargo test --locked` |
| MSRV tests | `cargo +1.85.0 test --locked` |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` |
| Supply chain | `cargo deny check` |

Run every gate except the MSRV tests with `./scripts/check.sh`.

The binary is behind the default `cli` feature. `cargo build
--no-default-features` builds the library alone, and `cargo test
--no-default-features` runs the library tests without the CLI.
