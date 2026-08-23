// SPDX-License-Identifier: MPL-2.0
//! Version of the linked native Prism library.

use prism_sys as sys;

/// The `(major, minor, patch)` version of the linked native Prism library.
///
/// Decoded from `prism_version()`'s packed `(major << 16) | (minor << 8) |
/// patch` encoding.
pub fn runtime_version() -> (u8, u8, u8) {
    // SAFETY: no preconditions.
    let packed = unsafe { sys::prism_version() };
    (
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        (packed & 0xFF) as u8,
    )
}

/// The linked native Prism library's version string, e.g. `"0.18.1"`.
pub fn runtime_version_string() -> &'static str {
    // SAFETY: `prism_version_string()` returns a pointer to a static,
    // NUL-terminated string literal baked into the library; valid for the
    // `'static` lifetime and never null.
    unsafe {
        core::ffi::CStr::from_ptr(sys::prism_version_string())
            .to_str()
            .unwrap_or_default()
    }
}
