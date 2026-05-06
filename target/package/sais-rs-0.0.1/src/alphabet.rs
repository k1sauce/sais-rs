//! Alphabet utilities and the internal `Symbol` trait that abstracts over
//! the two character types we encounter during SA-IS:
//!
//! * `u8` — the top-level call (`K = 256`).
//! * `i32` — the recursive reduced-alphabet call (named LMS substrings).

pub const K_BYTE: usize = 256;

/// Internal abstraction over the character type during SA-IS. Both impls are
/// trivial; the trait exists so the algorithm body can be written once for
/// both the top-level (`u8`) and recursive (`i32`) calls.
pub(crate) trait Symbol: Copy + Ord + Send + Sync {
    fn idx(self) -> usize;
}

impl Symbol for u8 {
    #[inline]
    fn idx(self) -> usize {
        self as usize
    }
}

impl Symbol for i32 {
    #[inline]
    fn idx(self) -> usize {
        debug_assert!(self >= 0, "negative reduced-alphabet symbol: {self}");
        self as usize
    }
}
