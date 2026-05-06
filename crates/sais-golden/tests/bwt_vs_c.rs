//! Phase 4 BWT goldens: byte-for-byte BWT + primary-index agreement vs C,
//! plus a serial-vs-parallel diff inside sais-rs.

use sais_golden::c_bwt;
use sais_rs::{bwt, bwt_parallel};

fn check(text: &[u8]) {
    let (ours, ours_p) = bwt(text).expect("bwt");
    let (theirs, theirs_p) = c_bwt(text);
    assert_eq!(
        ours_p,
        theirs_p,
        "primary index mismatch on len={}",
        text.len()
    );
    assert_eq!(ours, theirs, "BWT bytes mismatch on len={}", text.len());

    for threads in [0usize, 4] {
        let (par, par_p) = bwt_parallel(text, threads).expect("bwt_parallel");
        assert_eq!(
            par_p, ours_p,
            "parallel(threads={threads}) primary index drifted from serial"
        );
        assert_eq!(
            par, ours,
            "parallel(threads={threads}) BWT bytes drifted from serial"
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
fn all_nuls_4k() {
    check(&[0u8; 4096]);
}

#[test]
fn random_binary_64k() {
    check(&random_bytes(0x4242_4242, 64 * 1024));
}

#[test]
fn random_dna_1mb() {
    let bytes = random_bytes(0x1234_5678, 1024 * 1024);
    let dna: Vec<u8> = bytes.iter().map(|b| b"ACGT"[(*b as usize) % 4]).collect();
    check(&dna);
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
