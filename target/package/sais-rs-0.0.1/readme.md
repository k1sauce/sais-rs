# sais-rs

Pure-Rust SA-IS suffix array construction, BWT, and inverse BWT, with
[rayon](https://crates.io/crates/rayon) parallelism. Output is byte-for-byte
compatible with [libsais](https://github.com/IlyaGrebnov/libsais)
(Apache-2.0, Ilya Grebnov), validated by golden tests against `libsais-sys`
through 1 MB random corpora.

**Status: pre-alpha (v0.0.x).** 8-bit input only. Parallel induction lands
~5–10% over serial on Apple Silicon; bigger wins (parallel scatter, bigram
bucket trick) are on the v2 backlog. See [libsais.md](libsais.md) for the
design doc, port log, and the perf comparison vs `libsais-sys`.

## Requires nightly Rust

Uses `std::hint::prefetch_read` (tracking issue
[#146941](https://github.com/rust-lang/rust/issues/146941)) for the SA-IS
hot loop — gives ~16% serial / ~8% parallel on aarch64. Pin nightly via
`rust-toolchain.toml`:

```toml
[toolchain]
channel = "nightly"
```

When `hint_prefetch` stabilizes, this requirement goes away.

## Usage

```rust
use sais_rs::{suffix_array, bwt, unbwt, suffix_array_parallel};

let text = b"banana";
let sa = suffix_array(text)?;
let (compressed, primary) = bwt(text)?;
let recovered = unbwt(&compressed, primary)?;
assert_eq!(recovered, text);

// Parallel SA over 4 threads (or 0 to use the global rayon pool).
let par_sa = suffix_array_parallel(b"mississippi", 4)?;
```

## Build and test (workspace)

```bash
devbox shell
cargo test --workspace          # all goldens vs C, plus parallel-vs-serial diffs
cargo test --workspace --release
cargo clippy --workspace --all-targets -- -D warnings
```

The `sais-rs` crate has zero `unsafe` and no C dependency. The companion
`sais-golden` crate (`publish = false`) links `libsais-sys` for the
golden tests only.

## License

MIT OR Apache-2.0 (matching upstream libsais).
