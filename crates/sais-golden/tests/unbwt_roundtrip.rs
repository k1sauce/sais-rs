//! Phase 4 unBWT goldens: round-trip `bwt(t) → unbwt → t` and direct diff
//! against the C inverse on the BWT we generated.

use sais_golden::{c_bwt, c_unbwt};
use sais_rs::{bwt, unbwt};

fn check(text: &[u8]) {
    let (b, p) = bwt(text).expect("bwt");
    // Round-trip via our unbwt.
    let recovered = unbwt(&b, p).expect("unbwt");
    assert_eq!(recovered, text, "round-trip mismatch on len={}", text.len());
    // Cross-check: feed our BWT into the C inverse — must also recover.
    let recovered_c = c_unbwt(&b, p);
    assert_eq!(
        recovered_c,
        text,
        "C unbwt rejected our BWT on len={}",
        text.len()
    );
    // And: feed the C BWT into our inverse.
    let (cb, cp) = c_bwt(text);
    let recovered_from_c = unbwt(&cb, cp).expect("our unbwt on C bwt");
    assert_eq!(
        recovered_from_c,
        text,
        "our unbwt rejected C's BWT on len={}",
        text.len()
    );
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
fn random_binary_64k() {
    check(&random_bytes(0xCAFE_F00D, 64 * 1024));
}

#[test]
fn random_dna_1mb() {
    let bytes = random_bytes(0xFEED_BEEF, 1024 * 1024);
    let dna: Vec<u8> = bytes.iter().map(|b| b"ACGT"[(*b as usize) % 4]).collect();
    check(&dna);
}

#[test]
fn invalid_primary_index_errors() {
    // primary out of range → SaisError, not panic
    let result = unbwt(b"abc", 99);
    assert!(result.is_err());
}

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
