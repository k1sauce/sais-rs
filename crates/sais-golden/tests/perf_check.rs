//! One-shot perf comparison: serial SA-IS (us) vs libsais C on a 1 MB random
//! random binary input. Run via `cargo test --release -p libsais-golden
//! --test perf_check -- --nocapture --ignored`. Ignored by default to keep
//! routine `cargo test` fast.
//!
//! This is a sanity check, not a benchmark — single iteration, no warmup.

use std::time::Instant;

use sais_golden::c_suffix_array;
use sais_rs::{suffix_array, suffix_array_parallel};

fn random_bytes(seed: u64, n: usize) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xDEAD_BEEF_CAFE_BABE;
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        out.extend_from_slice(&z.to_le_bytes());
    }
    out.truncate(n);
    out
}

fn time<F: FnMut()>(mut f: F, n_iter: u32) -> f64 {
    // discard 1 warmup iter
    f();
    let start = Instant::now();
    for _ in 0..n_iter {
        f();
    }
    start.elapsed().as_secs_f64() * 1000.0 / n_iter as f64
}

#[test]
#[ignore]
fn rs_vs_c_1mb() {
    let t = random_bytes(0xBEEF, 1024 * 1024);
    let n_iter = 5;

    let serial_ms = time(
        || {
            let _ = suffix_array(&t).unwrap();
        },
        n_iter,
    );
    let par4_ms = time(
        || {
            let _ = suffix_array_parallel(&t, 4).unwrap();
        },
        n_iter,
    );
    let par_global_ms = time(
        || {
            let _ = suffix_array_parallel(&t, 0).unwrap();
        },
        n_iter,
    );
    let c_ms = time(
        || {
            let _ = c_suffix_array(&t);
        },
        n_iter,
    );

    eprintln!("1 MB random binary, {n_iter} iterations (after 1 warmup):");
    eprintln!(
        "  sais-rs serial    : {:>7.2} ms / iter ({:.2}x C)",
        serial_ms,
        serial_ms / c_ms
    );
    eprintln!(
        "  sais-rs par(4)    : {:>7.2} ms / iter ({:.2}x C, {:.2}x of serial)",
        par4_ms,
        par4_ms / c_ms,
        par4_ms / serial_ms
    );
    eprintln!(
        "  sais-rs par(0)    : {:>7.2} ms / iter ({:.2}x C, {:.2}x of serial)",
        par_global_ms,
        par_global_ms / c_ms,
        par_global_ms / serial_ms
    );
    eprintln!("  libsais-c  (1 thread): {:>7.2} ms / iter", c_ms);
}

#[test]
#[ignore]
fn rs_vs_c_8mb() {
    let t = random_bytes(0xFEED, 8 * 1024 * 1024);
    let n_iter = 3;

    let serial_ms = time(
        || {
            let _ = suffix_array(&t).unwrap();
        },
        n_iter,
    );
    let par4_ms = time(
        || {
            let _ = suffix_array_parallel(&t, 4).unwrap();
        },
        n_iter,
    );
    let par_global_ms = time(
        || {
            let _ = suffix_array_parallel(&t, 0).unwrap();
        },
        n_iter,
    );
    let c_ms = time(
        || {
            let _ = c_suffix_array(&t);
        },
        n_iter,
    );

    eprintln!("8 MB random binary, {n_iter} iterations (after 1 warmup):");
    eprintln!(
        "  sais-rs serial    : {:>7.2} ms / iter ({:.2}x C)",
        serial_ms,
        serial_ms / c_ms
    );
    eprintln!(
        "  sais-rs par(4)    : {:>7.2} ms / iter ({:.2}x C, {:.2}x of serial)",
        par4_ms,
        par4_ms / c_ms,
        par4_ms / serial_ms
    );
    eprintln!(
        "  sais-rs par(0)    : {:>7.2} ms / iter ({:.2}x C, {:.2}x of serial)",
        par_global_ms,
        par_global_ms / c_ms,
        par_global_ms / serial_ms
    );
    eprintln!("  libsais-c  (1 thread): {:>7.2} ms / iter", c_ms);
}
