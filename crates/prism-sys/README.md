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

## Static linking

`PRISM_STATIC=1` builds and links Prism as a static library. Because Prism is
then linked into *your* binary rather than resolving itself inside `prism.dll`,
`build.rs` has to reproduce what upstream's CMake package would have done, and
two consequences are visible from the outside.

**The archive is linked whole.** Upstream registers every built-in backend from
a file-scope static (`REGISTER_BACKEND*`). Nothing references those objects, so
an ordinary archive link drops them and leaves a working library with an empty
backend catalog. `build.rs` therefore links `prism` with `+whole-archive`;
`builtin_catalog_is_linked_in` in `crates/prism/tests` guards this.

**Prism's own native dependencies are replayed at the final link.** Everything
upstream links PRIVATE to the `prism` target resolves itself inside
`prism.dll`/`libprism.so` in a shared build. In a static one it has to be named
again when the consumer links, and we consume the CMake build tree directly
rather than through `find_package(prism)`, so `build.rs` does it: the generated
screen-reader import libraries and system libraries on Windows, the pkg-config
modules the build recorded in `prism-config.cmake` on Linux, and the SDK
frameworks on Apple. Miss this and the link fails on `spd_say` (Linux) or
`_AVSpeechUtteranceMaximumSpeechRate` (Apple) rather than on anything named
`prism_*`.

**Downstream binaries need three linker flags (Windows/MSVC only).** The
screen-reader DLLs Prism talks to (`ZDSRAPI_x64.dll`, `PCTKUSR.dll`, ...) ship
with the screen readers, not with Windows, so they must be delay-loaded or the
executable will not start (`STATUS_DLL_NOT_FOUND`). Cargo cannot pass link
arguments through a dependency, so `prism-sys` publishes the DLL names as
`links` metadata and each crate that links a binary repeats the step in its own
`build.rs`:

```rust
// build.rs of a crate that links a binary against a static Prism
fn main() {
    if let Ok(dlls) = std::env::var("DEP_PRISM_DELAYLOAD") {
        for dll in dlls.split(';').filter(|d| !d.is_empty()) {
            println!("cargo:rustc-link-arg=/delayload:{dll}");
        }
        println!("cargo:rustc-link-arg=/DELAY:unload");
        println!("cargo:rustc-link-arg=/ignore:4199");
    }
}
```

`DEP_PRISM_DELAYLOAD` only reaches *direct* dependents of `prism-sys`, so a
crate that depends on the safe `prism` wrapper has to depend on `prism-sys` as
well to see it. The variable is set only for a static MSVC build; the snippet
is a no-op otherwise. `crates/prism/build.rs` is exactly this.

## License

MPL-2.0, matching upstream Prism. The generated bindings are a derivative of
the MPL-2.0 `prism.h` header.
