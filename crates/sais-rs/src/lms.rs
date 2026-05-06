//! LMS substring extraction, lex-naming, and the recursion driver.

use crate::alphabet::Symbol;
use crate::classify::is_lms;

/// LMS positions in text order.
pub(crate) fn lms_positions(types: &[bool]) -> Vec<i32> {
    let n = types.len();
    let mut out = Vec::new();
    for i in 1..n {
        if is_lms(types, i) {
            out.push(i as i32);
        }
    }
    out
}

/// Compare two LMS substrings byte-by-byte until both reach the next LMS or
/// they diverge.
///
/// LMS substring at position `p` extends from `p` up to and including the next
/// LMS position (or the implicit sentinel at `n`). The implicit sentinel is
/// treated as a unique character smaller than every other; in particular, the
/// rightmost LMS substring (which terminates at `n`) is never equal to any
/// other LMS substring under this comparison.
fn lms_substrings_equal<C: Symbol>(t: &[C], types: &[bool], a: usize, b: usize) -> bool {
    if a == b {
        return true;
    }
    let n = t.len();
    let mut i = 0usize;
    loop {
        let pa = a + i;
        let pb = b + i;
        let a_at_end = pa >= n;
        let b_at_end = pb >= n;
        // The implicit sentinel is unique, so substrings ending at it are
        // never equal to substrings that continue past their start.
        if a_at_end || b_at_end {
            return false;
        }
        let a_lms = i > 0 && is_lms(types, pa);
        let b_lms = i > 0 && is_lms(types, pb);
        if a_lms && b_lms {
            return true;
        }
        if a_lms != b_lms {
            return false;
        }
        if t[pa] != t[pb] {
            return false;
        }
        i += 1;
    }
}

/// After the first L+S induction passes, the LMS substrings occupy SA in
/// sorted order. Walk SA in order, assigning a name to each LMS substring;
/// consecutive equal substrings share a name.
///
/// Returns `(reduced_string, alphabet_size)` where `reduced_string[i]` is the
/// name of the i-th LMS substring in **text order**.
pub(crate) fn name_lms<C: Symbol>(t: &[C], sa: &[i32], types: &[bool]) -> (Vec<i32>, usize) {
    let n = t.len();
    let mut name_at = vec![-1i32; n];
    let mut name: i32 = -1;
    let mut prev_lms: Option<usize> = None;
    for &v in sa {
        if v < 0 {
            continue;
        }
        let p = v as usize;
        if !is_lms(types, p) {
            continue;
        }
        match prev_lms {
            None => {
                name = 0;
                name_at[p] = name;
            }
            Some(prev) => {
                if !lms_substrings_equal(t, types, prev, p) {
                    name += 1;
                }
                name_at[p] = name;
            }
        }
        prev_lms = Some(p);
    }
    let alphabet_size = (name + 1) as usize;
    let mut reduced = Vec::new();
    for i in 0..n {
        if name_at[i] >= 0 {
            reduced.push(name_at[i]);
        }
    }
    (reduced, alphabet_size)
}
