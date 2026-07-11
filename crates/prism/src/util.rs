// SPDX-License-Identifier: MPL-2.0
//! Small FFI helpers shared across modules.

use crate::error::{Error, Result};
use std::ffi::{CStr, CString};

/// Convert a Rust `&str` into a `CString`, rejecting embedded NULs the same way
/// the upstream bindings do (an embedded NUL is an invalid parameter).
pub(crate) fn to_cstring(text: &str) -> Result<CString> {
    CString::new(text).map_err(|_| Error::InvalidParam)
}

/// Read a borrowed C string pointer into an owned `String`.
///
/// Returns [`Error::Internal`] if the library handed back a null pointer where
/// a string was promised.
///
/// # Safety
/// `ptr` must be null or point to a valid NUL-terminated string that lives for
/// the duration of this call.
pub(crate) unsafe fn owned_string(ptr: *const libc::c_char) -> Result<String> {
    if ptr.is_null() {
        return Err(Error::Internal);
    }
    Ok(CStr::from_ptr(ptr).to_string_lossy().into_owned())
}
