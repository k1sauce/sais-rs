//! Inverse BWT via LF-mapping over an *extended* (n+1)-length BWT.
//!
//! libsais's BWT format is `BWT(T·$)` with the sentinel `$` removed and
//! `T[n-1]` filling the gap at position 0. The "primary" returned by libsais
//! is the 0-based position of `$` in `BWT(T·$)`. To invert, we virtually
//! re-insert `$` at that position, run textbook LF mapping over the
//! (n+1)-length extended BWT (where `$` < every byte), and walk from primary.
//! The walk's first emission is `$` (discarded); the next n emissions are
//! `T` in reverse.
//!
//! This handles the difference between rotation-sort and suffix-sort that
//! breaks naive L-rank LF on the n-length libsais layout — see
//! [libsais.md](../../libsais.md) for the gory diagnostic story.

use crate::alphabet::K_BYTE;
use crate::error::SaisError;

pub fn unbwt(bwt: &[u8], primary_index: i32) -> Result<Vec<u8>, SaisError> {
    let n = bwt.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    // libsais primary is the 0-based position of `$` in BWT(T·$); valid
    // range is [1, n] (n+1 possible positions, but position 0 cannot hold $
    // because the smallest rotation `$T` always has L = T[n-1] ≠ $).
    if primary_index < 1 || (primary_index as usize) > n {
        return Err(SaisError::InvalidPrimaryIndex {
            primary: primary_index as i64,
            len: n,
        });
    }
    let primary = primary_index as usize;
    if n == 1 {
        return Ok(vec![bwt[0]]);
    }

    let m = n + 1;

    // Per-byte counts in libsais's bwt (= per-byte counts in BWT(T·$); the
    // single `$` doesn't appear in libsais's bwt and is accounted for separately).
    let mut counts = [0i32; K_BYTE];
    for &b in bwt {
        counts[b as usize] += 1;
    }
    // C[byte c] = (1 for $, which is smaller than every byte) + (count of
    // bytes < c in BWT(T·$)).
    let mut c_array = [0i32; K_BYTE];
    let mut sum = 1i32;
    for c in 0..K_BYTE {
        c_array[c] = sum;
        sum += counts[c];
    }

    // Extended-BWT view: ext_bwt[i] for i in 0..m, where i == primary is `$`.
    //
    //   i < primary  → libsais_bwt[i]
    //   i == primary → $   (handled by `None` in our `ext` closure)
    //   i > primary  → libsais_bwt[i - 1]
    let ext = |i: usize| -> Option<u8> {
        if i == primary {
            None
        } else if i < primary {
            Some(bwt[i])
        } else {
            Some(bwt[i - 1])
        }
    };

    // LF over ext_bwt. The single `$`-row maps to F position 0 (since `$`
    // is the smallest character).
    let mut occ = [0i32; K_BYTE];
    let mut lf = vec![0i32; m];
    for (i, slot) in lf.iter_mut().enumerate() {
        match ext(i) {
            None => *slot = 0, // L = $; F-position of $ is 0
            Some(c) => {
                let cu = c as usize;
                *slot = c_array[cu] + occ[cu];
                occ[cu] += 1;
            }
        }
    }

    // Walk from primary. The first emission is `$` (discard). The next n
    // emissions give T in reverse, so we collect into a buffer and reverse.
    let mut t = Vec::with_capacity(n);
    let mut idx = lf[primary] as usize; // skip the $ step
    for _ in 0..n {
        let c = ext(idx).expect("walk should not revisit the $ row before n steps");
        t.push(c);
        idx = lf[idx] as usize;
    }
    debug_assert_eq!(idx, primary, "walk should close back to primary");
    t.reverse();
    Ok(t)
}

pub fn unbwt_parallel(
    bwt: &[u8],
    primary_index: i32,
    threads: usize,
) -> Result<Vec<u8>, SaisError> {
    let _ = threads;
    unbwt(bwt, primary_index)
}
