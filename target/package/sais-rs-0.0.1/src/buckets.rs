//! Bucket counts and head/tail pointers.
//!
//! Head pointer of bucket `c`: index into SA where bucket starts.
//! Tail pointer of bucket `c`: index into SA one past where bucket ends.

use rayon::iter::ParallelIterator;
use rayon::slice::ParallelSlice;

use crate::alphabet::Symbol;
use crate::parallel::PARALLEL_MIN;

/// Count occurrences of each symbol. `counts.len() == k`.
pub(crate) fn count_symbols<C: Symbol>(t: &[C], k: usize) -> Vec<i32> {
    let mut counts = vec![0i32; k];
    for &c in t {
        counts[c.idx()] += 1;
    }
    counts
}

/// Parallel form: per-block private counts, element-wise reduce. Falls back
/// to serial below `PARALLEL_MIN` because fork-join overhead dominates on
/// small inputs.
pub(crate) fn count_symbols_parallel<C: Symbol>(t: &[C], k: usize) -> Vec<i32> {
    if t.len() < PARALLEL_MIN {
        return count_symbols(t, k);
    }
    let block = (t.len() / rayon::current_num_threads()).max(PARALLEL_MIN);
    t.par_chunks(block)
        .map(|chunk| {
            let mut local = vec![0i32; k];
            for &c in chunk {
                local[c.idx()] += 1;
            }
            local
        })
        .reduce(
            || vec![0i32; k],
            |mut acc, local| {
                for (a, l) in acc.iter_mut().zip(local.iter()) {
                    *a += l;
                }
                acc
            },
        )
}

/// `heads[c] = sum(counts[0..c])` — start of bucket `c`.
pub(crate) fn bucket_heads(counts: &[i32]) -> Vec<i32> {
    let mut heads = Vec::with_capacity(counts.len());
    let mut sum: i32 = 0;
    for &c in counts {
        heads.push(sum);
        sum = sum.checked_add(c).expect("bucket head overflow");
    }
    heads
}

/// `tails[c] = sum(counts[0..=c])` — one past end of bucket `c`.
pub(crate) fn bucket_tails(counts: &[i32]) -> Vec<i32> {
    let mut tails = Vec::with_capacity(counts.len());
    let mut sum: i32 = 0;
    for &c in counts {
        sum = sum.checked_add(c).expect("bucket tail overflow");
        tails.push(sum);
    }
    tails
}
