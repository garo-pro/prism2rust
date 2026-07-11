// SPDX-License-Identifier: MPL-2.0
#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]
#![allow(clippy::all)]
#![doc = include_str!("../README.md")]

//! Raw, unsafe FFI bindings for the Prism C ABI.
//!
//! This crate is a thin `-sys` layer: it exposes the C functions, structs, and
//! constants from `include/prism.h` of the pinned upstream release verbatim.
//! Prefer the safe [`prism`](../prism/index.html) crate for application code.
//!
//! The bindings are either the checked-in `bindings_pregenerated.rs` (default)
//! or regenerated from the vendored header via the `bindgen` feature. Both are
//! kept in sync by the `update-bridge` maintenance skill.

// The bridge itself.
#[cfg(feature = "bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
#[cfg(not(feature = "bindgen"))]
include!("bindings_pregenerated.rs");

#[cfg(test)]
mod layout_sanity {
    use super::*;

    // A handful of ABI anchors that must hold regardless of how the bindings
    // were produced. bindgen also emits its own exhaustive layout tests when
    // the `bindgen` feature is enabled.
    #[test]
    fn backend_id_is_u64() {
        assert_eq!(core::mem::size_of::<PrismBackendId>(), 8);
    }

    #[test]
    fn ok_is_zero() {
        assert_eq!(PRISM_OK, 0);
    }

    #[test]
    fn config_version_matches() {
        assert_eq!(PRISM_CONFIG_VERSION, 3);
    }

    // Note: the `PRISM_BACKEND_*` ids are `#define`s using `UINT64_C(...)`,
    // which bindgen's macro evaluator does not expand, so they are absent from
    // the raw FFI by design. They are surfaced (and value-checked) via
    // `prism_types::BackendId`.
}
