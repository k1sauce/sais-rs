#!/usr/bin/env bash
# v1 definition-of-done check. Run from workspace root.
set -euo pipefail

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --release
