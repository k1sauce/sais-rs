//! L/S/LMS type classification.
//!
//! Convention: `types[i] == true` ⇔ position `i` is S-type. The implicit
//! sentinel at position `n` is conceptually the smallest character ($), so
//! position `n-1` is always L-type.
//!
//! Storage is `Vec<bool>` — one byte per position. Two alternatives were
//! tried and measured *slower* on Apple Silicon: a `Vec<u64>` packed bitset
//! (shift+mask cost dominates because the M-series has 16-24 MB L2 and
//! `Vec<bool>` already fits) and a `Vec<u8>` newtype wrapper (small but
//! consistent ~5% regression, likely from inlining edges around the
//! accessor). On x86 with small L2 the bitset would probably win — revisit
//! per-platform if it ever matters there.

use crate::alphabet::Symbol;

/// One right-to-left pass producing the L/S classification.
pub(crate) fn classify<C: Symbol>(t: &[C]) -> Vec<bool> {
    let n = t.len();
    let mut types = vec![false; n];
    if n == 0 {
        return types;
    }
    // Position n-1 is L-type → false (already initialized).
    let mut prev_is_s = false;
    for i in (0..n - 1).rev() {
        let cur_is_s = match t[i].cmp(&t[i + 1]) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal => prev_is_s,
        };
        types[i] = cur_is_s;
        prev_is_s = cur_is_s;
    }
    types
}

/// Leftmost-S: position `i > 0` is LMS iff it is S-type and its predecessor
/// is L-type. Position 0 is never LMS by definition.
#[inline]
pub(crate) fn is_lms(types: &[bool], i: usize) -> bool {
    i > 0 && types[i] && !types[i - 1]
}
