---
name: update-bridge
description: Regenerate the raw FFI ("the bridge") from the pinned Prism header and reconcile the safe wrappers and shared types with any upstream API changes, then run the full test gate. Use after bumping the submodule (via update-prism), or whenever the C header/ABI has changed and the Rust bindings must be brought back in sync.
---

# update-bridge

Bring the Rust bindings back into sync with `external/prism/include/prism.h`.
The "bridge" is `crates/prism-sys` (raw FFI) + the safe mappings in
`crates/prism` + the shared constants in `crates/prism-types`.

## Prerequisites

- The submodule is already at the intended commit (normally set by
  `update-prism`).
- `libclang` is available for regeneration. On Windows it is typically at
  `C:/Program Files/LLVM/bin`; export `LIBCLANG_PATH` to its directory.

## Steps

1. **Regenerate the raw FFI** into the checked-in file:
   ```bash
   # Set LIBCLANG_PATH if bindgen can't find libclang automatically.
   PRISM_SYS_NO_NATIVE=1 PRISM_SYS_UPDATE_PREGENERATED=1 \
     cargo build -p prism-sys --features bindgen
   ```
   This rewrites `crates/prism-sys/src/bindings_pregenerated.rs`.

2. **Review the FFI diff** — this is the authoritative change list:
   ```bash
   git diff -- crates/prism-sys/src/bindings_pregenerated.rs
   ```
   Look for: new/removed/renamed `prism_*` functions, changed struct fields,
   new/renamed/reordered enum values, and `PRISM_CONFIG_VERSION` changes.

3. **Reconcile `crates/prism-types`** (`src/lib.rs`) for any changed value types:
   - New/removed `PrismError` codes → add/remove `Error` variants, update
     `error_code`, `from_code`, `code`, `message`, and the round-trip tests.
   - New/changed `PRISM_BACKEND_*` ids → update `BackendId` consts + `ALL` +
     `well_known_name` + the spot-value test.
   - New `PrismBackendFeature` bits → add flags with the exact bit position and
     extend `feature_bits_match_header_positions`.
   - Changed `PRISM_CONFIG_VERSION` → update `CONFIG_VERSION` + its test.

4. **Reconcile `crates/prism`** (safe wrappers) for function/vtable changes:
   - New top-level function → add a method on `Context`/`Backend`.
   - New `PrismBackendVTable` slot → add a `CustomBackend` trait method (with a
     sensible default) **and** a matching trampoline in `registry.rs`, then wire
     it into `build_vtable`.
   - Changed signatures → update the corresponding wrapper + `sys` call site.

5. **Add or update tests for everything touched** (mandatory, per CLAUDE.md):
   - Value types → assertions in `prism-types` (run without the native lib).
   - Wrappers/vtable → extend `crates/prism/tests/custom_backend.rs` so the new
     path is driven end-to-end through the real library.

6. **Run the gate:**
   ```bash
   cargo fmt --all
   cargo fmt --all --check
   PRISM_SYS_NO_NATIVE=1 cargo clippy --workspace --all-targets -- -D warnings
   cargo test -p prism-types --all-features
   cargo test --workspace         # needs the C++23 toolchain + CMake
   ```
   If the native toolchain is unavailable locally, at minimum get the first
   three green and rely on CI's `native` job for the integration tests — but say
   so explicitly in your summary.

7. **Report** the ABI changes you found, what you changed in response, and the
   test results. If you could not run `cargo test --workspace` locally, state it.

## Notes

- **Never hand-edit** `bindings_pregenerated.rs`; always regenerate it.
- If bindgen drops a `#define` (e.g. macros wrapped in `UINT64_C(...)` are not
  evaluated), surface that constant through `prism-types` instead and note it,
  as is already done for the `PRISM_BACKEND_*` ids.
