// SPDX-License-Identifier: MPL-2.0
//
// Build script for `prism-sys`. Two responsibilities:
//
//   1. FFI bindings ("the bridge"):
//        * Default: no work — `src/lib.rs` includes the checked-in
//          `src/bindings_pregenerated.rs`, so downstream users need neither
//          libclang nor a network fetch.
//        * `--features bindgen`: regenerate the FFI from the vendored header
//          into `OUT_DIR/bindings.rs`. If `PRISM_SYS_UPDATE_PREGENERATED=1`,
//          the freshly generated file is also copied over the checked-in
//          `src/bindings_pregenerated.rs`. This is what the `update-bridge`
//          maintenance skill runs after bumping the submodule.
//
//   2. Native library: configure + build the vendored Prism C/C++23 library
//      with the `cmake` crate and emit the link directives, unless linking is
//      overridden or skipped (see the environment variables below).
//
// Environment variables (all optional):
//   PRISM_SYS_NO_NATIVE=1  Skip building/linking the native library entirely.
//                          Used for `cargo check`, docs, and pure-logic tests.
//   PRISM_LIB_DIR=<path>   Link a prebuilt Prism instead of building it. The
//                          directory must contain the import/static library.
//   PRISM_STATIC=1         The (prebuilt or built) library is static.
//   PRISM_SYS_UPDATE_PREGENERATED=1  (bindgen feature) overwrite the checked-in
//                          pregenerated bindings with the fresh output.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let header_dir = manifest_dir.join("../../external/prism/include");
    let header = header_dir.join("prism.h");

    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", header.display());
    println!("cargo:rerun-if-env-changed=PRISM_SYS_NO_NATIVE");
    println!("cargo:rerun-if-env-changed=PRISM_LIB_DIR");
    println!("cargo:rerun-if-env-changed=PRISM_STATIC");

    // Re-export the include dir so dependent -sys consumers can find prism.h.
    println!("cargo:include={}", header_dir.display());

    generate_bindings(&manifest_dir, &header_dir);
    link_native(&manifest_dir);
}

/// (Re)generate FFI bindings when the `bindgen` feature is enabled; otherwise
/// this is a no-op and `src/bindings_pregenerated.rs` is used verbatim.
#[cfg(feature = "bindgen")]
fn generate_bindings(manifest_dir: &Path, header_dir: &Path) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // prism.h includes a `prism_version.h` that upstream normally generates
    // from `include/prism_version.h.in` via CMake `configure_file`. The
    // native build (below) gets that for free from CMake; bindgen parses
    // the header directly, so reproduce it here from the crate version
    // (which tracks the pinned upstream release, see PRISM_PIN.toml).
    let generated_include_dir = write_generated_version_header(&out_dir);

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", header_dir.display()))
        .clang_arg(format!("-I{}", generated_include_dir.display()))
        // Only surface the Prism surface, not the transitced system headers.
        .allowlist_item("[Pp][Rr][Ii][Ss][Mm].*")
        .allowlist_item("PRISM_.*")
        .prepend_enum_name(false)
        .default_enum_style(bindgen::EnumVariation::Consts)
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .layout_tests(true)
        .use_core()
        .ctypes_prefix("::libc")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("failed to generate Prism FFI bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("failed to write generated bindings to OUT_DIR");

    if env::var_os("PRISM_SYS_UPDATE_PREGENERATED").is_some() {
        let dst = manifest_dir.join("src/bindings_pregenerated.rs");
        bindings
            .write_to_file(&dst)
            .expect("failed to refresh checked-in pregenerated bindings");
        println!(
            "cargo:warning=refreshed checked-in bindings: {}",
            dst.display()
        );
    }
}

#[cfg(not(feature = "bindgen"))]
fn generate_bindings(_manifest_dir: &Path, _header_dir: &Path) {
    // Default path: the checked-in `src/bindings_pregenerated.rs` is used
    // directly by `src/lib.rs`. Nothing to do at build time.
}

/// Write a `prism_version.h` derived from the crate version into
/// `OUT_DIR/generated_include`, mirroring what upstream's CMake
/// `configure_file(include/prism_version.h.in ...)` produces, and return
/// that directory so it can be added to the bindgen include path.
#[cfg(feature = "bindgen")]
fn write_generated_version_header(out_dir: &Path) -> PathBuf {
    use std::fs;

    let major = env::var("CARGO_PKG_VERSION_MAJOR").unwrap();
    let minor = env::var("CARGO_PKG_VERSION_MINOR").unwrap();
    let patch = env::var("CARGO_PKG_VERSION_PATCH").unwrap();
    let version = env::var("CARGO_PKG_VERSION").unwrap();

    let dir = out_dir.join("generated_include");
    fs::create_dir_all(&dir).expect("failed to create generated include dir");
    let contents = format!(
        "// SPDX-License-Identifier: MPL-2.0\n\
         //\n\
         // GENERATED FILE - DO NOT EDIT.\n\
         // Produced by prism-sys/build.rs (mirrors upstream's\n\
         // include/prism_version.h.in, substituted from the crate version).\n\
         \n\
         #ifndef PRISM_VERSION_H\n\
         #define PRISM_VERSION_H\n\
         \n\
         #define PRISM_VERSION_MAJOR {major}\n\
         #define PRISM_VERSION_MINOR {minor}\n\
         #define PRISM_VERSION_PATCH {patch}\n\
         #define PRISM_VERSION_STRING \"{version}\"\n\
         \n\
         #endif\n"
    );
    fs::write(dir.join("prism_version.h"), contents)
        .expect("failed to write generated prism_version.h");
    dir
}

/// Build or locate the native Prism library and emit link directives.
fn link_native(manifest_dir: &Path) {
    // Skip entirely for check-only / docs / pure-logic test runs.
    if env::var_os("PRISM_SYS_NO_NATIVE").is_some()
        || env::var_os("DOCS_RS").is_some()
        || cfg!(docsrs)
    {
        println!(
            "cargo:warning=prism-sys: native build skipped (PRISM_SYS_NO_NATIVE/DOCS_RS); \
             extern symbols will be unresolved until a Prism library is linked"
        );
        return;
    }

    let is_static = env::var_os("PRISM_STATIC").is_some();

    if let Some(dir) = env::var_os("PRISM_LIB_DIR") {
        // Use a prebuilt library.
        println!(
            "cargo:rustc-link-search=native={}",
            Path::new(&dir).display()
        );
    } else {
        // Build the vendored library from source with CMake.
        let prism_src = manifest_dir.join("../../external/prism");
        assert!(
            prism_src.join("CMakeLists.txt").exists(),
            "vendored Prism sources not found at {}. Did you run \
             `git submodule update --init --recursive`?",
            prism_src.display()
        );

        let mut cfg = cmake::Config::new(&prism_src);
        cfg.define("PRISM_ENABLE_TESTS", "OFF")
            .define("PRISM_ENABLE_DEMOS", "OFF")
            .define("PRISM_ENABLE_GDEXTENSION", "OFF")
            .define("PRISM_ENABLE_LINTING", "OFF");
        // Prefer a static library so the test/bin artifacts are self-contained.
        if is_static {
            cfg.define("BUILD_SHARED_LIBS", "OFF");
        }
        let dst = cfg.build();

        println!(
            "cargo:rustc-link-search=native={}",
            dst.join("lib").display()
        );
        println!(
            "cargo:rustc-link-search=native={}",
            dst.join("bin").display()
        );
        println!("cargo:root={}", dst.display());
    }

    // The C ABI header uses __declspec(dllimport) unless PRISM_STATIC is set;
    // that macro affects the C side only. On the Rust side we just name the
    // symbol. `prism` is the CMake OUTPUT_NAME for the library on every target.
    let kind = if is_static { "static" } else { "dylib" };
    println!("cargo:rustc-link-lib={kind}=prism");
}
