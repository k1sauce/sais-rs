//! Safe wrappers around `libsais-sys` for golden tests. The only `unsafe`
//! in the project lives here.

use libsais_sys::libsais as c;

/// Compute the suffix array of `text` using the upstream C library.
///
/// Panics on FFI error or input larger than `i32::MAX`. Tests-only.
pub fn c_suffix_array(text: &[u8]) -> Vec<i32> {
    if text.is_empty() {
        return Vec::new();
    }
    let n = i32::try_from(text.len()).expect("text length exceeds i32::MAX");
    let mut sa = vec![0i32; text.len()];
    // SAFETY: `text` and `sa` are valid for `n` elements; `freq` is null
    // (libsais documents this is allowed); `fs = 0` is documented as fine.
    let ret = unsafe { c::libsais(text.as_ptr(), sa.as_mut_ptr(), n, 0, std::ptr::null_mut()) };
    assert_eq!(ret, 0, "libsais returned error code {ret} for n={n}");
    sa
}

/// Compute BWT of `text` via libsais. Returns `(bwt_bytes, primary_index)`.
pub fn c_bwt(text: &[u8]) -> (Vec<u8>, i32) {
    if text.is_empty() {
        return (Vec::new(), 0);
    }
    let n = i32::try_from(text.len()).expect("text length exceeds i32::MAX");
    let mut bwt = vec![0u8; text.len()];
    let mut a = vec![0i32; text.len()];
    // SAFETY: T and U are valid for n bytes (U may equal T per docs); A is
    // valid for n + fs (fs=0); freq is null.
    let ret = unsafe {
        c::libsais_bwt(
            text.as_ptr(),
            bwt.as_mut_ptr(),
            a.as_mut_ptr(),
            n,
            0,
            std::ptr::null_mut(),
        )
    };
    assert!(ret >= 0, "libsais_bwt returned error code {ret} for n={n}");
    (bwt, ret)
}

/// Inverse BWT via libsais. The temporary array must be `n + 1` size per
/// upstream docs.
pub fn c_unbwt(bwt: &[u8], primary: i32) -> Vec<u8> {
    if bwt.is_empty() {
        return Vec::new();
    }
    let n = i32::try_from(bwt.len()).expect("bwt length exceeds i32::MAX");
    let mut out = vec![0u8; bwt.len()];
    let mut a = vec![0i32; bwt.len() + 1];
    // SAFETY: T valid for n bytes; U valid for n bytes; A valid for n+1 i32;
    // freq is null; i is the primary index.
    let ret = unsafe {
        c::libsais_unbwt(
            bwt.as_ptr(),
            out.as_mut_ptr(),
            a.as_mut_ptr(),
            n,
            std::ptr::null(),
            primary,
        )
    };
    assert_eq!(ret, 0, "libsais_unbwt returned error code {ret} for n={n}");
    out
}
