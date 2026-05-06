//! SA → BWT derivation in libsais's exact byte layout.
//!
//! Convention (matching `libsais_bwt`):
//!   * Output length is `n`.
//!   * `bwt[0] = text[n-1]`, the wrapped character of the primary row.
//!   * Canonical BWT chars (each `text[(SA[i]-1) mod n]`) for rows *before*
//!     the primary occupy `bwt[1..primary]`.
//!   * Canonical BWT chars for rows *after* the primary occupy
//!     `bwt[primary..n]` unshifted.
//!   * Primary index is 1-based: `(arg(SA[i]==0)) + 1`, in `[1, n]`.
//!
//! Both serial and parallel forms agree byte-for-byte with libsais's
//! `libsais_bwt` for u8 input.

use rayon::iter::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator};

use crate::error::SaisError;
use crate::index::SaIndex;
use crate::parallel::{PARALLEL_MIN, with_threads};
use crate::sa::{suffix_array_into, suffix_array_parallel_into};

/// Compute BWT of `text`. Returns `(bwt_bytes, primary_index)`.
pub fn bwt(text: &[u8]) -> Result<(Vec<u8>, i32), SaisError> {
    let n = text.len();
    let mut sa = vec![0i32; n];
    suffix_array_into(text, &mut sa)?;
    Ok(bwt_from_sa_serial(text, &sa))
}

/// Parallel BWT.
pub fn bwt_parallel(text: &[u8], threads: usize) -> Result<(Vec<u8>, i32), SaisError> {
    let n = text.len();
    let mut sa = vec![0i32; n];
    suffix_array_parallel_into(text, &mut sa, threads)?;
    Ok(with_threads(threads, || bwt_from_sa_parallel(text, &sa)))
}

/// Caller-provided SA scratch (overwritten with the SA as a side effect).
pub fn bwt_with_sa<I: SaIndex>(
    text: &[u8],
    sa_scratch: &mut [I],
) -> Result<(Vec<u8>, I), SaisError> {
    let n = text.len();
    if sa_scratch.len() != n {
        return Err(SaisError::BufferLen {
            expected: n,
            got: sa_scratch.len(),
        });
    }
    suffix_array_into(text, sa_scratch)?;
    if n == 0 {
        return Ok((Vec::new(), I::ZERO));
    }
    if n == 1 {
        return Ok((vec![text[0]], I::ONE));
    }
    let mut bwt_buf = vec![0u8; n];
    bwt_buf[0] = text[n - 1];
    let mut primary_0: Option<usize> = None;
    for (i, slot) in sa_scratch.iter().enumerate() {
        let s = slot.as_usize();
        if s == 0 {
            primary_0 = Some(i);
            break;
        }
        bwt_buf[i + 1] = text[s - 1];
    }
    let p0 = primary_0.expect("SA must contain 0");
    for (i, slot) in sa_scratch.iter().enumerate().skip(p0 + 1) {
        let s = slot.as_usize();
        debug_assert!(s != 0);
        bwt_buf[i] = text[s - 1];
    }
    Ok((bwt_buf, I::from_usize(p0 + 1)))
}

fn bwt_from_sa_serial(text: &[u8], sa: &[i32]) -> (Vec<u8>, i32) {
    let n = text.len();
    if n == 0 {
        return (Vec::new(), 0);
    }
    if n == 1 {
        return (vec![text[0]], 1);
    }
    let mut bwt = vec![0u8; n];
    bwt[0] = text[n - 1];
    // Forward scan, writing canonical-BWT[i] → bwt[i+1] until we hit primary.
    let mut p0: Option<usize> = None;
    for (i, &s) in sa.iter().enumerate() {
        if s == 0 {
            p0 = Some(i);
            break;
        }
        bwt[i + 1] = text[(s - 1) as usize];
    }
    let p0 = p0.expect("SA must contain 0");
    // After primary: write canonical-BWT[i] → bwt[i] (no shift).
    for (i, &s) in sa.iter().enumerate().skip(p0 + 1) {
        debug_assert!(s > 0);
        bwt[i] = text[(s - 1) as usize];
    }
    (bwt, (p0 + 1) as i32)
}

fn bwt_from_sa_parallel(text: &[u8], sa: &[i32]) -> (Vec<u8>, i32) {
    let n = text.len();
    if n == 0 {
        return (Vec::new(), 0);
    }
    if n == 1 {
        return (vec![text[0]], 1);
    }
    if n < PARALLEL_MIN {
        return bwt_from_sa_serial(text, sa);
    }
    // Find primary serially — single linear pass on i32 is faster than
    // forking a parallel scan for typical inputs.
    let p0 = sa.iter().position(|&s| s == 0).expect("SA must contain 0");

    let mut bwt = vec![0u8; n];
    bwt[0] = text[n - 1];
    bwt[1..].par_iter_mut().enumerate().for_each(|(j, slot)| {
        // Slot bwt[j+1] sources from sa[j] (j<p0) or sa[j+1] (j>=p0,
        // skipping the primary row whose char is already at bwt[0]).
        let src = if j < p0 { j } else { j + 1 };
        let s = sa[src];
        debug_assert!(s > 0);
        *slot = text[(s - 1) as usize];
    });
    (bwt, (p0 + 1) as i32)
}
