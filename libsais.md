# libsais-rs — design doc and port log

A from-scratch Rust port of [libsais](https://github.com/IlyaGrebnov/libsais).
The C library is the de-facto fastest open-source SA/BWT/LCP toolkit
(SA-IS + OpenMP), but ships ~1.5 MB of C across four alphabet-width-specialized
source files. The crates.io ecosystem only has FFI wrappers; competing
pure-Rust crates use slower algorithms. This crate aims to fill that gap.

## Goals

- Pure Rust in the release artifact. C tolerated only as a dev-only golden
  reference (see [crates/libsais-golden](crates/libsais-golden)).
- Idiomatic Rust API: byte slices, `Result`, owned-or-buffer outputs.
- Rayon-parallel from day one, mirroring upstream's OpenMP structure.
- Correctness anchored to byte-for-byte agreement with the C library on
  randomized + structured corpora.

## v1 scope

- **Input:** 8-bit (`&[u8]`) only.
- **Outputs:** suffix array, BWT (with primary index), unBWT.
- **Index type:** `i32`, behind a `SaIndex` trait so a future i64 variant is
  a type swap rather than a rewrite.
- **Parallelism:** rayon, conservative per-bucket-range strategy.

Deferred to v2: 16/32/64-bit alphabets, generalized SA, LCP/PLCP,
atomic-radix induction, criterion benches as primary signal.

## Architecture

Two crates in a workspace:

| Crate | Purpose | C dep? |
|---|---|---|
| `libsais-rs` | Public library | **No** — never reachable from `Cargo.toml` |
| `libsais-golden` | Validation harness | Yes (`libsais-sys`); `publish = false` |

The split exists so that `cargo install libsais-rs` never needs a C toolchain.
All `unsafe` in the project is contained to
[crates/libsais-golden/src/lib.rs](crates/libsais-golden/src/lib.rs).

## Module layout (`libsais-rs`)

| Module | Responsibility |
|---|---|
| `index` | `SaIndex` trait + `i32` (and `i64`) impls |
| `alphabet` | K=256 bucket sizing helpers |
| `classify` | L/S/LMS classification (right-to-left, parallel via reconciliation) |
| `buckets` | Bucket counts, head/tail pointers (parallel `count_bytes`) |
| `lms` | LMS substring extraction, lex-naming, recursion driver |
| `induce` | Induced L+S passes — perf-critical core |
| `sa` | Top-level orchestrator, public `suffix_array_*` |
| `bwt` | SA → BWT (embarrassingly parallel) |
| `unbwt` | LF-mapping inverse |
| `parallel` | rayon helpers, thread-pool scoping |
| `util` | tiny helpers, debug invariants |

## Dependency plan

| Concern | Crate | Notes |
|---|---|---|
| Parallelism | `rayon` 1.10 | OpenMP equivalent |
| Errors | `thiserror` 2 | derive `Error` on `SaisError` |
| Validation FFI | `libsais-sys` =0.2.0 | golden crate only; pinned exactly |
| Test glue | `anyhow` 1 | golden crate only |
| Bench (v2) | `criterion` 0.5 | gated behind feature on golden crate |

Deliberately not used: `num_traits` (small surface, roll our own),
`tokio`/`async-std` (no async surface), `divsufsort` (different algorithm).

## Phase log

| Phase | Status |
|---|---|
| 0 — Scaffold | ✅ done |
| 1 — Naive SA + golden harness | ✅ done (10 tests vs C, all green) |
| 2 — Serial SA-IS | ✅ done (17 tests vs C through 1 MB; debug + release; clippy clean) |
| 3 — Rayon plumbing + parallel `count_symbols` + diff harness | ✅ done (see below) |
| 3.5 — Block-parallel `induce_l` / `induce_s` (gather parallel, scatter serial) | ✅ done (37/37 goldens green; ~1.2× over serial at 8 MB) |
| 4 — BWT + unBWT | ✅ done (10 BWT-vs-C tests, 10 round-trip tests, debug + release, clippy clean) |
| 5 — Polish | ✅ done (doctests, README example, perf check) |
| **v2 backlog** | parallel `classify`, parallel induction (atomic-radix), parallel LMS scatter |

### Phase 3 scope reduction

The plan called for rayon-parallel induction from day one. After implementation
analysis I scoped it down. What landed:

- Parallel `count_symbols` (`par_chunks` × per-thread `[i32; k]` → element-wise
  reduce). Falls back to serial below `PARALLEL_MIN = 1<<14`.
- Threadpool plumbing in [parallel.rs](crates/libsais-rs/src/parallel.rs)
  (`with_threads(0, ..)` = global pool; `with_threads(N, ..)` = scoped pool).
- `Mode::{Serial, Parallel}` parameter on `sais_inner`. Recursion always uses
  `Serial` (reduced inputs are at most n/2; fork-join overhead would dominate).
- All 17 goldens now diff parallel-vs-serial on `threads={0, 4}` in addition
  to byte-for-byte vs C.

### Phase 3.5 — block-parallel induced sort

Both `induce_l` and `induce_s` now have block-parallel siblings
(`induce_l_parallel`, `induce_s_parallel`) using the libsais block scheme:

1. Walk SA looking for a contiguous run of non-empty slots — a "block".
2. **Gather (parallel)**: chunk the block across rayon threads. Each thread
   reads its sub-range, type-classifies, and accumulates `(j, dest_bucket)`
   pairs into a per-thread cache.
3. **Scatter (serial)**: walk the caches in left-to-right (or right-to-left
   for S-pass) chunk order, applying inductions to the shared SA.

The correctness invariant: bucket head/tail pointers always reference an
empty slot. Since the block extends only over non-empty slots, no induction
write from this block can land inside the block — writes go to slots
outside `[block_start, block_end)`. Reads within the block therefore see a
stable snapshot.

Effective threads per block are capped at `block_size / PAR_BLOCK_MIN` so
the global rayon pool doesn't over-subscribe small blocks (fork-join cost
otherwise dominates the gather win).

What's still on the v2 backlog:

- **Parallel scatter.** Currently scatter is serial, so peak speedup is
  bounded by the gather/scatter ratio. To unlock the rest we'd switch SA
  to `Vec<AtomicI32>` (or take the `forbid(unsafe_code)` hit and use raw
  pointers + a safety doc). Either route should push 4-thread perf closer
  to libsais's headline numbers.
- **Parallel `classify`.** The L/S right-to-left propagation is only
  sequentially dependent on equal-character runs, but those runs can span
  chunk boundaries; the chunked + reconciliation scheme needs care to
  handle long equal-runs across chunks. Marginal win for the risk.
- **Parallel LMS tail-scatter.** The first scatter is order-independent
  and could be partitioned by bucket; the second scatter needs sorted
  order within each bucket. Skipped for v1; small fraction of total time.

**v1 perf expectation:** parallel API matches serial on tiny inputs, modest
win on inputs where `count_symbols` is non-negligible (≥ 4 MB, small
alphabet). v1 will not approach upstream libsais's 4-core throughput.

**Measured perf** (release, nightly toolchain, 3+ run averages on Apple
Silicon M-series, 8 MB random binary):

| Impl | ms / iter | vs C |
|---|---|---|
| `libsais-rs` serial, no prefetch | 370 | 2.78× |
| `libsais-rs` serial, prefetch on | **309** | 2.43× |
| `libsais-rs` par(4), no prefetch | 329 | 2.46× |
| `libsais-rs` par(4), prefetch on | **302** | 2.36× |
| `libsais-sys` (1 thread) | 130 | — |

Each independent optimization is real but modest:

- **Software prefetch** (`std::hint::prefetch_read` from
  [induce.rs](crates/libsais-rs/src/induce.rs), guarded by the `hint_prefetch`
  feature gate, hence the nightly pin): +16% on serial induce, +8% on
  parallel. Apple Silicon's hardware prefetcher absorbs most of the
  random-access cost on its own — x86 servers (where libsais was tuned)
  typically see larger wins from the same hint.
- **Block-parallel induction**: ~5-8% on 4 cores once prefetch is on.
  Bottleneck is the serial scatter; lifting it requires atomic SA slots
  (tried — see below).

Reproducible via `cargo test --release -p libsais-golden --test perf_check -- --ignored`.

Run-to-run variance on the dev machine is ±10-20% — measure 3+ runs
before drawing conclusions about a tuning change. Cross-day comparisons
are unreliable due to thermal/scheduler state; same-compilation A/B is
the only signal that consistently survives.

### Optimizations tried and *reverted* on Apple Silicon

The trade space looks different on M-series than on the x86 servers
where libsais was tuned. Each of these had a sound theoretical case but
didn't measurably help here:

- **`Vec<u64>` bitset for `types`** (1 bit per position vs 1 byte). Goal:
  shrink working set 8× to fit L2. Reality: M-series has 16-24 MB L2, so
  the original `Vec<bool>` already fits, and the per-access shift+mask
  cost more than it saves. **+15% slower** than `Vec<bool>` on 8 MB
  random binary.
- **Newtype wrapper over `Vec<u8>`** (preparing for a future bitset
  swap). Goal: keep call sites stable while swapping representations.
  Reality: ~5% slower than raw `Vec<bool>`, possibly from inlining
  edges. Reverted; raw `Vec<bool>` it is.
- **Parallel scatter via `AtomicI32::from_mut_slice`** (nightly,
  `feature(atomic_from_mut)`). Goal: drop the serial-scatter bottleneck
  by having every thread store via `Relaxed` atomics into its disjoint
  slot range, with offsets computed by a tiny serial reduce. Reality:
  the extra fork-join cycle per block (gather + scatter = two parallel
  sections instead of one) plus atomic-store overhead canceled the
  parallelism win on aarch64. Apple Silicon's store buffer handles the
  serial scatter's short bursts very well. There was also a correctness
  regression at `PAR_BLOCK_MAX > 1<<16` that I didn't fully diagnose
  before reverting. May still help on x86 — the implementation lives
  in git history if anyone wants to revisit.

### What's left (still on the v2 backlog)

The remaining gap to libsais (~2.4× C → ~1.0× C) on Apple Silicon would
need either:

- **Bigram bucket trick** (libsais's L-pass writes both a position and a
  packed type+next-char tag in one slot, eliminating the `types[]`
  random access). Substantive refactor — different bucket layout,
  encoded high-bit flags on SA values. Unsafe-free in principle, real
  work in practice.
- **Bounds-check elision in the hot loop**. The unprovable `j = sa[i] - 1`
  index can't have its `types[j]` / `t[j]` checks elided at compile
  time. Lifting `forbid(unsafe_code)` on [induce.rs](crates/libsais-rs/src/induce.rs)
  for `get_unchecked` calls would buy ~5-10%. Outside the project's
  current scope.

## Implementation notes

**Sentinel handling in SA-IS.** libsais and this port operate on `t[0..n]`
without an explicit sentinel. Position `n-1` is therefore always L-type
(since `t[n-1] > $`), but standard SA-IS pseudocode only seeds the L-pass
from already-placed LMS positions. For inputs whose LMS list is empty
(`"abc"`, `"cba"`, `"aaa"`) the L-pass would be a no-op without an explicit
seed. We seed `n-1` at the head of its bucket *inside* `induce_l`, sharing
the `heads` pointer state with the regular induction loop — keeping the
seed in a separate function (with its own local `heads`) caused the first
regular induction to overwrite the seed slot. See
[induce.rs](crates/libsais-rs/src/induce.rs).

**libsais BWT layout.** libsais's BWT for length-n input is `BWT(T·$)` with
the sentinel `$` removed: `bwt[0] = T[n-1]` (the L of the smallest rotation
`$T`), and the remaining n-1 positions are `BWT(T·$)` with `$` deleted. The
returned "primary" is the 0-based position where `$` lived in `BWT(T·$)`,
guaranteed to be in `[1, n]`.

**Why the textbook L-rank LF doesn't directly invert it.** A naive LF on
the n-byte libsais layout (treating it as the L-column of a length-n
rotation BWT) is broken: when SA-sort and rotation-sort disagree (as they
can when one suffix is a prefix of another), the L-rank LF's
"k-th occurrence of c in L corresponds to the k-th smallest c-prefixed
suffix" property fails for some rows. Concretely: random 300-byte inputs
hit this with first-divergence at indices like 16-19, producing LF cycles
of length ≪ n and garbage output. Diagnosed by comparing `T[(SA[i]-1) mod n]`
against a brute-force rotation BWT.

**Inverse approach.** [unbwt.rs](crates/libsais-rs/src/unbwt.rs) virtually
re-inserts `$` at `primary` to materialize an (n+1)-length extended BWT
where `$` < every byte. Standard L-rank LF on the extended view is correct
(rotation BWT of `T·$` has all distinct rotations). The walk starts at
`primary` (whose L is `$`), discards the first emission, and collects the
next `n` chars in reverse → `T`.

Update the status column as phases land. Each phase ends with
`cargo test --workspace` green.
