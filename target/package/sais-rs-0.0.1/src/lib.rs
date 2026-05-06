//! Pure-Rust SA-IS suffix array construction, BWT, and inverse BWT.
//! Output is byte-for-byte compatible with
//! [libsais](https://github.com/IlyaGrebnov/libsais) (validated by ~37
//! golden tests through 1 MB random corpora).
//!
//! v0.0.x scope: 8-bit input only. Requires nightly Rust for
//! `std::hint::prefetch_read` (tracking issue
//! [#146941](https://github.com/rust-lang/rust/issues/146941)).
//!
//! ```
//! use sais_rs::{suffix_array, bwt, unbwt};
//!
//! let text = b"banana";
//! let sa = suffix_array(text).unwrap();
//! assert_eq!(sa, vec![5, 3, 1, 0, 4, 2]);
//!
//! let (b, primary) = bwt(text).unwrap();
//! let recovered = unbwt(&b, primary).unwrap();
//! assert_eq!(recovered, text);
//! ```
//!
//! Parallel variants live alongside the serial ones — pass `threads = 0` to
//! reuse the global rayon pool, or any other count to spin a scoped pool:
//!
//! ```
//! use sais_rs::suffix_array_parallel;
//! let sa = suffix_array_parallel(b"mississippi", 4).unwrap();
//! ```

#![forbid(unsafe_code)]
#![feature(hint_prefetch)]
// tracking issue #146941; safe `std::hint::prefetch_*`
// SA-IS is fundamentally index-arithmetic: bucket pointers, type lookups,
// and recursion driver loops all index multiple parallel arrays at the same
// position. iter()/enumerate() rewrites obscure that intent.
#![allow(clippy::needless_range_loop)]

mod alphabet;
mod buckets;
mod bwt;
mod classify;
mod error;
mod index;
mod induce;
mod lms;
mod parallel;
mod sa;
mod unbwt;
mod util;

pub use crate::bwt::{bwt, bwt_parallel, bwt_with_sa};
pub use crate::error::SaisError;
pub use crate::index::SaIndex;
pub use crate::sa::{
    suffix_array, suffix_array_into, suffix_array_parallel, suffix_array_parallel_into,
    suffix_array_with,
};
pub use crate::unbwt::{unbwt, unbwt_parallel};

// `sa::suffix_array_into` and `suffix_array_parallel_into` are referenced by
// `bwt.rs` via crate::sa. The pub use above keeps them in the public API too.
