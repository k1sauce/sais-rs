//! Centralized rayon plumbing.
//!
//! `with_threads(0, f)` runs `f` on the global rayon pool; any other count
//! builds a scoped pool and runs `f` inside `pool.install`. This is the only
//! place in the crate that touches `ThreadPoolBuilder`.

use rayon::ThreadPoolBuilder;

/// Run `f` on a rayon pool sized to `threads`. `threads = 0` uses the global
/// pool (no scoped pool is constructed).
pub(crate) fn with_threads<R, F>(threads: usize, f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    if threads == 0 {
        return f();
    }
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("rayon pool build");
    pool.install(f)
}

/// Lower bound below which parallel passes fall back to serial. Tuned so the
/// fork-join overhead of `par_chunks` doesn't outweigh the work.
pub(crate) const PARALLEL_MIN: usize = 1 << 14;
