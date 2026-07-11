// SPDX-License-Identifier: MPL-2.0
//! Error handling for the safe Prism API.

use prism_sys as sys;

/// The error type for all fallible Prism operations.
///
/// Re-exported from [`prism_types`] so it is identical across the FFI (`-sys`)
/// and safe layers.
pub use prism_types::Error;

/// A `Result` whose error is a Prism [`Error`].
pub type Result<T> = core::result::Result<T, Error>;

/// Convert a raw `PrismError` code returned across the FFI into a [`Result`].
#[inline]
pub(crate) fn check(code: sys::PrismError) -> Result<()> {
    // `sys::PrismError` is `c_int`, which is always 32-bit on Rust targets.
    Error::check(code)
}

/// Fetch the upstream C library's human-readable string for an error code.
///
/// Falls back to [`Error::message`] if the pointer is null. This is exposed so
/// callers can surface the library's own wording when it is richer than the
/// static Rust message.
pub fn error_string(error: Error) -> String {
    // SAFETY: `prism_error_string` accepts any code and returns either a
    // pointer to a static NUL-terminated string or null.
    let ptr = unsafe { sys::prism_error_string(error.code() as sys::PrismError) };
    if ptr.is_null() {
        return error.message().to_owned();
    }
    // SAFETY: non-null pointer to a static NUL-terminated C string.
    let cstr = unsafe { core::ffi::CStr::from_ptr(ptr) };
    cstr.to_string_lossy().into_owned()
}
