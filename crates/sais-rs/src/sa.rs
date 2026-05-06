//! Top-level SA-IS orchestrator.
//!
//! Implements the two-stage SA-IS:
//!   1. Approximate-sort LMS substrings via initial L+S induction.
//!   2. Name LMS substrings; recurse on the reduced string if names are not
//!      unique; place LMS in correct order at bucket tails.
//!   3. Final L+S induction fills in the remaining suffixes.
//!
//! No explicit sentinel is appended to `text` (matching libsais's API). The
//! implicit sentinel at position `n` is treated as the smallest character;
//! position `n-1` is consequently always L-type. The L-pass is bootstrapped
//! by placing `n-1` at the head of its bucket — without this seed,
//! sentinel-less inputs without any LMS (`"abc"`, `"cba"`, `"aaa"`) would
//! produce an empty SA.

use crate::alphabet::{K_BYTE, Symbol};
use crate::buckets::{bucket_tails, count_symbols, count_symbols_parallel};
use crate::classify::{classify, is_lms};
use crate::error::SaisError;
use crate::index::SaIndex;
use crate::induce::{induce_l, induce_l_parallel, induce_s, induce_s_parallel};
use crate::lms::{lms_positions, name_lms};
use crate::parallel::with_threads;

#[derive(Copy, Clone)]
enum Mode {
    Serial,
    Parallel,
}

/// Compute the suffix array of `text` (single-threaded).
pub fn suffix_array(text: &[u8]) -> Result<Vec<i32>, SaisError> {
    suffix_array_with::<i32>(text)
}

/// Generic variant — kept for future i64 alphabet support.
pub fn suffix_array_with<I: SaIndex>(text: &[u8]) -> Result<Vec<I>, SaisError> {
    let mut sa = vec![I::ZERO; text.len()];
    suffix_array_into(text, &mut sa)?;
    Ok(sa)
}

/// In-place: caller provides an output buffer of length `text.len()`.
pub fn suffix_array_into<I: SaIndex>(text: &[u8], sa: &mut [I]) -> Result<(), SaisError> {
    if sa.len() != text.len() {
        return Err(SaisError::BufferLen {
            expected: text.len(),
            got: sa.len(),
        });
    }
    let n = text.len();
    if n == 0 {
        return Ok(());
    }
    if I::try_from_usize(n - 1).is_none() {
        return Err(SaisError::InputTooLarge(n));
    }
    if i32::try_from(n).is_err() {
        return Err(SaisError::InputTooLarge(n));
    }

    // v1 runs the algorithm on i32 internally regardless of `I`. The extra
    // n*4 bytes is acceptable; transmuting `&mut [I]` to `&mut [i32]` would
    // require unsafe, which is forbidden in this crate.
    let mut scratch = vec![0i32; n];
    sais_inner::<u8>(text, &mut scratch, K_BYTE, Mode::Serial);
    for (slot, val) in sa.iter_mut().zip(scratch.iter()) {
        debug_assert!(*val >= 0);
        *slot = I::from_usize(*val as usize);
    }
    Ok(())
}

/// Parallel variant. `threads = 0` uses the global rayon pool.
pub fn suffix_array_parallel(text: &[u8], threads: usize) -> Result<Vec<i32>, SaisError> {
    let mut sa = vec![0i32; text.len()];
    suffix_array_parallel_into(text, &mut sa, threads)?;
    Ok(sa)
}

pub fn suffix_array_parallel_into<I: SaIndex>(
    text: &[u8],
    sa: &mut [I],
    threads: usize,
) -> Result<(), SaisError> {
    if sa.len() != text.len() {
        return Err(SaisError::BufferLen {
            expected: text.len(),
            got: sa.len(),
        });
    }
    let n = text.len();
    if n == 0 {
        return Ok(());
    }
    if I::try_from_usize(n - 1).is_none() || i32::try_from(n).is_err() {
        return Err(SaisError::InputTooLarge(n));
    }
    let mut scratch = vec![0i32; n];
    with_threads(threads, || {
        sais_inner::<u8>(text, &mut scratch, K_BYTE, Mode::Parallel);
    });
    for (slot, val) in sa.iter_mut().zip(scratch.iter()) {
        debug_assert!(*val >= 0);
        *slot = I::from_usize(*val as usize);
    }
    Ok(())
}

/// Core SA-IS recursion. Generic over the symbol type `C` so the same body
/// runs on `&[u8]` (top level) and `&[i32]` (reduced alphabet during recursion).
///
/// In `Mode::Parallel`, only `count_symbols` runs in parallel. Classify and
/// induce stay serial in v1 — see [libsais.md](../../libsais.md) for why.
/// Recursive calls always run in `Mode::Serial` (reduced inputs are small).
fn sais_inner<C: Symbol>(t: &[C], sa: &mut [i32], k: usize, mode: Mode) {
    let n = t.len();
    debug_assert_eq!(sa.len(), n);
    if n == 0 {
        return;
    }
    if n == 1 {
        sa[0] = 0;
        return;
    }

    let types = classify(t);
    let counts = match mode {
        Mode::Serial => count_symbols(t, k),
        Mode::Parallel => count_symbols_parallel(t, k),
    };

    // 1. Place LMS positions at the tails of their buckets in arbitrary order.
    sa.fill(-1);
    {
        let mut tails = bucket_tails(&counts);
        for i in 1..n {
            if is_lms(&types, i) {
                let c = t[i].idx();
                tails[c] -= 1;
                sa[tails[c] as usize] = i as i32;
            }
        }
    }

    // 2. Initial L-pass (which seeds `n-1` internally) and S-pass —
    //    approximate-sorts the LMS substrings.
    match mode {
        Mode::Serial => {
            induce_l(sa, t, &types, &counts);
            induce_s(sa, t, &types, &counts);
        }
        Mode::Parallel => {
            induce_l_parallel(sa, t, &types, &counts);
            induce_s_parallel(sa, t, &types, &counts);
        }
    }

    // 3. Name LMS substrings into a reduced string.
    let (reduced, alphabet_size) = name_lms(t, sa, &types);
    let lms_in_text_order = lms_positions(&types);
    debug_assert_eq!(reduced.len(), lms_in_text_order.len());
    let lms_count = reduced.len();

    // 4. Recurse (or invert directly if names are already unique).
    let reduced_sa = if alphabet_size < lms_count {
        let mut sa_red = vec![0i32; lms_count];
        // Recursion stays serial; reduced inputs are at most n/2 and the
        // fork-join overhead would dominate.
        sais_inner::<i32>(&reduced, &mut sa_red, alphabet_size, Mode::Serial);
        sa_red
    } else {
        let mut sa_red = vec![0i32; lms_count];
        for (i, &name) in reduced.iter().enumerate() {
            sa_red[name as usize] = i as i32;
        }
        sa_red
    };

    // 5. Re-place LMS at bucket tails in correct sorted order. Iterate the
    //    sorted reduced SA in reverse so consecutive `tails[c] -= 1` writes
    //    place the largest LMS at the largest tail-slot first.
    sa.fill(-1);
    {
        let mut tails = bucket_tails(&counts);
        for &reduced_idx in reduced_sa.iter().rev() {
            let pos = lms_in_text_order[reduced_idx as usize];
            let c = t[pos as usize].idx();
            tails[c] -= 1;
            sa[tails[c] as usize] = pos;
        }
    }

    // 6. Final induction.
    match mode {
        Mode::Serial => {
            induce_l(sa, t, &types, &counts);
            induce_s(sa, t, &types, &counts);
        }
        Mode::Parallel => {
            induce_l_parallel(sa, t, &types, &counts);
            induce_s_parallel(sa, t, &types, &counts);
        }
    }
}
