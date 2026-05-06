use core::fmt::Debug;
use core::ops::{Add, AddAssign, Sub, SubAssign};

/// Index integer type for SA construction. v1 ships only `i32`; `i64` impl
/// exists so a future variant is a type swap rather than a rewrite.
pub trait SaIndex:
    Copy
    + Default
    + Debug
    + Eq
    + Ord
    + Send
    + Sync
    + Add<Output = Self>
    + Sub<Output = Self>
    + AddAssign
    + SubAssign
    + 'static
{
    const ZERO: Self;
    const ONE: Self;
    const MAX: Self;

    fn from_usize(x: usize) -> Self;
    fn try_from_usize(x: usize) -> Option<Self>;
    fn as_usize(self) -> usize;
    fn as_i64(self) -> i64;
}

impl SaIndex for i32 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
    const MAX: Self = i32::MAX;

    #[inline]
    fn from_usize(x: usize) -> Self {
        debug_assert!(x as u64 <= i32::MAX as u64, "i32 SA index overflow: {x}");
        x as i32
    }
    #[inline]
    fn try_from_usize(x: usize) -> Option<Self> {
        i32::try_from(x).ok()
    }
    #[inline]
    fn as_usize(self) -> usize {
        debug_assert!(self >= 0, "negative SA index used as offset: {self}");
        self as usize
    }
    #[inline]
    fn as_i64(self) -> i64 {
        self as i64
    }
}

impl SaIndex for i64 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
    const MAX: Self = i64::MAX;

    #[inline]
    fn from_usize(x: usize) -> Self {
        debug_assert!(x as u128 <= i64::MAX as u128, "i64 SA index overflow: {x}");
        x as i64
    }
    #[inline]
    fn try_from_usize(x: usize) -> Option<Self> {
        i64::try_from(x).ok()
    }
    #[inline]
    fn as_usize(self) -> usize {
        debug_assert!(self >= 0, "negative SA index used as offset: {self}");
        self as usize
    }
    #[inline]
    fn as_i64(self) -> i64 {
        self
    }
}
