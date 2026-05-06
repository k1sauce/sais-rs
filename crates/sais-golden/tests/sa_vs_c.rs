//! Phase 1 golden tests: byte-for-byte SA agreement on tiny corpora,
//! anchored by the naive reference impl in `sais-rs`.

use sais_golden::c_suffix_array;
use sais_rs::{suffix_array, suffix_array_parallel};

fn check(text: &[u8]) {
    let serial = suffix_array(text).expect("serial");
    let theirs = c_suffix_array(text);
    assert_eq!(
        serial,
        theirs,
        "serial-vs-C mismatch on input len={}",
        text.len()
    );
    // Diff parallel against serial on global pool and a small custom pool.
    for threads in [0usize, 4] {
        let par = suffix_array_parallel(text, threads).expect("parallel");
        assert_eq!(
            par,
            serial,
            "parallel(threads={threads}) vs serial mismatch on input len={}",
            text.len()
        );
    }
}

#[test]
fn empty() {
    check(b"");
}

#[test]
fn single_byte() {
    check(b"a");
}

#[test]
fn two_bytes_distinct() {
    check(b"ab");
}

#[test]
fn two_bytes_same() {
    check(b"aa");
}

#[test]
fn banana() {
    check(b"banana");
}

#[test]
fn abracadabra() {
    check(b"abracadabra");
}

#[test]
fn mississippi() {
    check(b"mississippi");
}

#[test]
fn alpha_smoke_with_nul() {
    check(b"banana\0abracadabra\0mississippi");
}

#[test]
fn all_same_64() {
    check(&[b'A'; 64]);
}

#[test]
fn binary_with_zero_byte() {
    check(b"\x00\x01\x00\x01\x02\x00");
}

#[test]
fn fibonacci_string() {
    // Classic SA-IS stress case: deep LMS recursion.
    let mut a = b"a".to_vec();
    let mut b = b"b".to_vec();
    for _ in 0..20 {
        let next = [b.clone(), a].concat();
        a = b;
        b = next;
    }
    check(&b);
}

#[test]
fn random_binary_16k() {
    check(&random_bytes(0xDEAD_BEEF, 16 * 1024));
}

#[test]
fn random_binary_256k() {
    check(&random_bytes(0xC0DE_F00D, 256 * 1024));
}

#[test]
fn random_dna_1mb() {
    // Small alphabet → high bucket pressure, deeper recursion.
    let bytes = random_bytes(0x1234_5678, 1024 * 1024);
    let dna: Vec<u8> = bytes.iter().map(|b| b"ACGT"[(*b as usize) % 4]).collect();
    check(&dna);
}

#[test]
fn random_ascii_1mb() {
    let bytes = random_bytes(0xABCD_1234, 1024 * 1024);
    let ascii: Vec<u8> = bytes.iter().map(|b| 32 + (*b % 95)).collect();
    check(&ascii);
}

#[test]
fn all_nuls_4k() {
    check(&[0u8; 4096]);
}

#[test]
fn repeating_4byte_pattern_64k() {
    let pat = b"\x01\x02\x03\x04";
    let mut v = Vec::with_capacity(64 * 1024);
    while v.len() < 64 * 1024 {
        v.extend_from_slice(pat);
    }
    check(&v);
}

/// Deterministic xorshift-style pseudo-random byte generator. Avoids pulling
/// in `rand` for tests.
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
