# prism-sys

Raw, unsafe FFI bindings for [Prism](https://github.com/ethindp/prism)'s C ABI,
pinned to the upstream release recorded in `PRISM_PIN.toml` at the repo root.

Most users want the safe [`prism`](../prism) crate instead.

## How the bindings are produced

By default the crate compiles the checked-in `src/bindings_pregenerated.rs`, so
**no libclang and no network access are required** to build it. Enable the
`bindgen` feature to regenerate the bindings from the vendored header at build
time; the `update-bridge` maintenance skill uses that mode (with
`PRISM_SYS_UPDATE_PREGENERATED=1`) to refresh the checked-in file whenever the
pinned upstream version changes.

## Building the native library

`build.rs` builds the vendored C/C++23 library via CMake and links it. Prism
requires a C++23 compiler. Useful environment variables:

| Variable | Effect |
| --- | --- |
| `PRISM_SYS_NO_NATIVE=1` | Skip building/linking the native library (for `cargo check`, docs, and pure-logic tests). |
| `PRISM_LIB_DIR=<path>` | Link a prebuilt Prism from `<path>` instead of building it. |
| `PRISM_STATIC=1` | Treat the (built or prebuilt) library as static. |

## License

MPL-2.0, matching upstream Prism. The generated bindings are a derivative of
the MPL-2.0 `prism.h` header.
