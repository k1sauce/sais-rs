//! Induced-sort L-pass and S-pass (serial + block-parallel).
//!
//! The L-pass walks SA left-to-right, inducing L-type positions into bucket
//! heads. The S-pass walks right-to-left, inducing S-type positions into
//! bucket tails.
//!
//! ## Block-parallel scheme (matches libsais)
//!
//! Both passes have a hidden invariant that makes parallelism safe: the
//! bucket head/tail pointer always points at an *empty* slot. So if we walk
//! SA looking for a contiguous run of *non-empty* slots — a "block" — and
//! process that block in parallel, no induction-write from this block can
//! land in the block itself (writes go to head/tail = an empty slot, which
//! by definition of the block is outside the block). Within the block,
//! parallel gather sees a stable snapshot.
//!
//! ## v1.7 — parallel gather + serial scatter
//!
//! 1. Gather (parallel): each thread scans its sub-range, type-classifies,
//!    and accumulates `(j, dest_bucket)` pairs into a local cache.
//! 2. Scatter (serial): walk the caches in chunk order (left-to-right for
//!    L-pass, right-to-left for S-pass) and apply inductions to the shared
//!    SA slice.
//!
//! ## Why not parallel scatter?
//!
//! Tried it. The implementation lives in git history at this file's edit
//! preceding this comment, using `AtomicI32::from_mut_slice` (nightly,
//! `feature(atomic_from_mut)`) plus per-bucket count + prefix-sum reduce.
//! On Apple Silicon the win didn't materialize: the extra fork-join cycle
//! per block (we'd be doing two parallel sections instead of one) and
//! atomic-store overhead canceled the parallelism gain. There was also a
//! correctness regression at `PAR_BLOCK_MAX > 1<<16` that I didn't fully
//! diagnose. Serial scatter is fast enough on aarch64 because the writes
//! to bucket heads are short, sequential bursts that the store buffer
//! handles well. May be worth revisiting on x86 servers.

use std::hint::{Locality, prefetch_read};

use rayon::iter::ParallelIterator;
use rayon::slice::ParallelSlice;

use crate::alphabet::Symbol;
use crate::buckets::{bucket_heads, bucket_tails};

/// Look-ahead distance for prefetching `types[j]` and `t[j]` where
/// `j = sa[i + DIST] - 1`. 64 matches libsais's distance and is roughly
/// a memory-latency window at typical clocks.
const PREFETCH_DIST: usize = 64;

/// Toggle the prefetch hints below. Set false to A/B against the
/// no-prefetch baseline on the same compilation. Measured win on Apple
/// Silicon (M-series): ~16% on serial induce, ~8% on parallel.
const ENABLE_PREFETCH: bool = true;

/// Below this block size, the gather/scatter overhead beats the parallelism
/// win and we fall back to the serial inner loop within the block.
const PAR_BLOCK_MIN: usize = 1024;

/// Hard cap on a single block. Bounds memory used by the per-thread cache
/// and gives the outer driver a chance to pick up newly-written slots.
const PAR_BLOCK_MAX: usize = 1 << 16;

/// L-pass: seed `n-1` at the head of its bucket, then induce all other
/// L-type positions.
pub(crate) fn induce_l<C: Symbol>(sa: &mut [i32], t: &[C], types: &[bool], counts: &[i32]) {
    let n = sa.len();
    if n == 0 {
        return;
    }
    let mut heads = bucket_heads(counts);
    seed_last_l(sa, t, types, &mut heads);
    for i in 0..n {
        if ENABLE_PREFETCH && i + PREFETCH_DIST < n {
            let v_far = sa[i + PREFETCH_DIST];
            if v_far > 0 {
                let j_far = (v_far - 1) as usize;
                prefetch_read(&types[j_far], Locality::L1);
                prefetch_read(&t[j_far], Locality::L1);
            }
        }
        let v = sa[i];
        if v <= 0 {
            continue;
        }
        let j = (v - 1) as usize;
        if types[j] {
            continue;
        }
        let c = t[j].idx();
        let slot = heads[c] as usize;
        sa[slot] = j as i32;
        heads[c] += 1;
    }
}

/// S-pass: induce S-type positions from already-placed entries; walks SA
/// right-to-left.
pub(crate) fn induce_s<C: Symbol>(sa: &mut [i32], t: &[C], types: &[bool], counts: &[i32]) {
    let n = sa.len();
    let mut tails = bucket_tails(counts);
    for i in (0..n).rev() {
        if ENABLE_PREFETCH && i >= PREFETCH_DIST {
            let v_far = sa[i - PREFETCH_DIST];
            if v_far > 0 {
                let j_far = (v_far - 1) as usize;
                prefetch_read(&types[j_far], Locality::L1);
                prefetch_read(&t[j_far], Locality::L1);
            }
        }
        let v = sa[i];
        if v <= 0 {
            continue;
        }
        let j = (v - 1) as usize;
        if !types[j] {
            continue;
        }
        let c = t[j].idx();
        tails[c] -= 1;
        let slot = tails[c] as usize;
        sa[slot] = j as i32;
    }
}

/// Block-parallel L-pass. Same final state as `induce_l`.
pub(crate) fn induce_l_parallel<C: Symbol>(
    sa: &mut [i32],
    t: &[C],
    types: &[bool],
    counts: &[i32],
) {
    let n = sa.len();
    if n == 0 {
        return;
    }
    let mut heads = bucket_heads(counts);
    seed_last_l(sa, t, types, &mut heads);

    let mut i = 0usize;
    while i < n {
        // Skip leading empty slots (true `-1` only; `sa[i] == 0` is the
        // valid SA entry for position 0 and counts as non-empty).
        while i < n && sa[i] < 0 {
            i += 1;
        }
        if i >= n {
            break;
        }

        // Extend block until we hit an empty slot or the size cap.
        let block_end_max = (i + PAR_BLOCK_MAX).min(n);
        let mut block_end = i + 1;
        while block_end < block_end_max && sa[block_end] >= 0 {
            block_end += 1;
        }
        let block_size = block_end - i;

        if block_size < PAR_BLOCK_MIN {
            // Tiny block — serial inner loop is faster than rayon fork-join.
            for k_idx in i..block_end {
                let v = sa[k_idx];
                if v <= 0 {
                    continue;
                }
                let j = (v - 1) as usize;
                if types[j] {
                    continue;
                }
                let c = t[j].idx();
                let slot = heads[c] as usize;
                sa[slot] = j as i32;
                heads[c] += 1;
            }
            i = block_end;
            continue;
        }

        let n_threads = rayon::current_num_threads()
            .min(block_size / PAR_BLOCK_MIN)
            .max(1);
        let chunk_size = block_size.div_ceil(n_threads).max(1);

        // Phase 1 (parallel): gather (j, dest_bucket) pairs.
        let caches: Vec<Vec<(i32, u32)>> = sa[i..block_end]
            .par_chunks(chunk_size)
            .map(|chunk| {
                let mut cache = Vec::with_capacity(chunk.len());
                for k_idx in 0..chunk.len() {
                    if ENABLE_PREFETCH && k_idx + PREFETCH_DIST < chunk.len() {
                        let v_far = chunk[k_idx + PREFETCH_DIST];
                        if v_far > 0 {
                            let j_far = (v_far - 1) as usize;
                            prefetch_read(&types[j_far], Locality::L1);
                            prefetch_read(&t[j_far], Locality::L1);
                        }
                    }
                    let v = chunk[k_idx];
                    if v <= 0 {
                        continue;
                    }
                    let j = (v - 1) as usize;
                    if types[j] {
                        continue;
                    }
                    cache.push((j as i32, t[j].idx() as u32));
                }
                cache
            })
            .collect();

        // Phase 2 (serial scatter): apply caches in chunk order.
        for cache in &caches {
            for &(j, c) in cache {
                let cu = c as usize;
                let slot = heads[cu] as usize;
                sa[slot] = j;
                heads[cu] += 1;
            }
        }
        i = block_end;
    }
}

/// Block-parallel S-pass. Same final state as `induce_s`.
pub(crate) fn induce_s_parallel<C: Symbol>(
    sa: &mut [i32],
    t: &[C],
    types: &[bool],
    counts: &[i32],
) {
    let n = sa.len();
    if n == 0 {
        return;
    }
    let mut tails = bucket_tails(counts);

    let mut i = n;
    while i > 0 {
        while i > 0 && sa[i - 1] < 0 {
            i -= 1;
        }
        if i == 0 {
            break;
        }

        let block_start_min = i.saturating_sub(PAR_BLOCK_MAX);
        let mut block_start = i - 1;
        while block_start > block_start_min && sa[block_start - 1] >= 0 {
            block_start -= 1;
        }
        let block_size = i - block_start;

        if block_size < PAR_BLOCK_MIN {
            for idx in (block_start..i).rev() {
                let v = sa[idx];
                if v <= 0 {
                    continue;
                }
                let j = (v - 1) as usize;
                if !types[j] {
                    continue;
                }
                let c = t[j].idx();
                tails[c] -= 1;
                let slot = tails[c] as usize;
                sa[slot] = j as i32;
            }
            i = block_start;
            continue;
        }

        let n_threads = rayon::current_num_threads()
            .min(block_size / PAR_BLOCK_MIN)
            .max(1);
        let chunk_size = block_size.div_ceil(n_threads).max(1);

        // Phase 1 (parallel): gather. Within each chunk we iterate in
        // reverse so cached entries are in right-to-left source-position
        // order, matching the serial S-pass walk.
        let caches: Vec<Vec<(i32, u32)>> = sa[block_start..i]
            .par_chunks(chunk_size)
            .map(|chunk| {
                let mut cache = Vec::with_capacity(chunk.len());
                for k_idx in (0..chunk.len()).rev() {
                    if ENABLE_PREFETCH && k_idx >= PREFETCH_DIST {
                        let v_far = chunk[k_idx - PREFETCH_DIST];
                        if v_far > 0 {
                            let j_far = (v_far - 1) as usize;
                            prefetch_read(&types[j_far], Locality::L1);
                            prefetch_read(&t[j_far], Locality::L1);
                        }
                    }
                    let v = chunk[k_idx];
                    if v <= 0 {
                        continue;
                    }
                    let j = (v - 1) as usize;
                    if !types[j] {
                        continue;
                    }
                    cache.push((j as i32, t[j].idx() as u32));
                }
                cache
            })
            .collect();

        // Phase 2 (serial scatter): apply caches in REVERSE chunk order so
        // the rightmost chunk's inductions decrement tails first.
        for cache in caches.iter().rev() {
            for &(j, c) in cache {
                let cu = c as usize;
                tails[cu] -= 1;
                let slot = tails[cu] as usize;
                sa[slot] = j;
            }
        }
        i = block_start;
    }
}

#[inline]
fn seed_last_l<C: Symbol>(sa: &mut [i32], t: &[C], types: &[bool], heads: &mut [i32]) {
    let n = sa.len();
    debug_assert!(!types[n - 1], "position n-1 must be L-type");
    let c = t[n - 1].idx();
    let slot = heads[c] as usize;
    sa[slot] = (n - 1) as i32;
    heads[c] += 1;
}
