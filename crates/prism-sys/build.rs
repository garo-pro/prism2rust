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
use std::fs;
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
    let is_msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    // Where the CMake package was installed, when we did the installing. The
    // generated `share/prism/prism-config.cmake` under it records the
    // pkg-config modules a static build needs; see `pkgconfig_modules`.
    let install_prefix: Option<PathBuf>;

    if let Some(dir) = env::var_os("PRISM_LIB_DIR") {
        // Use a prebuilt library.
        println!(
            "cargo:rustc-link-search=native={}",
            Path::new(&dir).display()
        );
        // A prebuilt tree is usually laid out as <prefix>/lib, so look one
        // level up for the CMake package; if it is not there we simply find
        // no modules and say so.
        install_prefix = Path::new(&dir).parent().map(Path::to_path_buf);
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
        // Always state the library kind: `cmake` reuses `$OUT_DIR/build`
        // across runs, and CMake would otherwise keep whatever a previous
        // build cached, so flipping PRISM_STATIC would silently do nothing.
        cfg.define("BUILD_SHARED_LIBS", if is_static { "OFF" } else { "ON" });
        if is_msvc {
            // rustc links the *release, dynamic* MSVC CRT (msvcrt.lib).
            // Upstream defaults to the static CRT, which for a static Prism
            // means the final link fails on missing `*_dbg` CRT symbols, so
            // pin the runtime to match. The cache entry we pre-seed here wins
            // over upstream's non-FORCE `set(... CACHE ...)`.
            cfg.define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreadedDLL");
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
        install_prefix = Some(dst);
    }

    // The C ABI header uses __declspec(dllimport) unless PRISM_STATIC is set;
    // that macro affects the C side only. On the Rust side we just name the
    // symbol. `prism` is the CMake OUTPUT_NAME for the library on every target.
    if is_static {
        // `+whole-archive` is mandatory, not an optimization: upstream registers
        // every built-in backend from a file-scope `BackendRegistrar` static
        // (`REGISTER_BACKEND*` in source/backend_catalog.h). Nothing references
        // those objects, so a normal archive link drops them and the registry
        // comes up empty. Pulling the whole archive keeps the registrars.
        println!("cargo:rustc-link-lib=static:+whole-archive=prism");
    } else {
        println!("cargo:rustc-link-lib=dylib=prism");
    }

    // A shared Prism resolves its own dependencies inside prism.dll. A static
    // one does not: everything upstream links PRIVATE to the `prism` target
    // has to be repeated at the final link, because we consume the CMake
    // build directly rather than through `find_package(prism)`. (Screen-reader
    // DLLs need no special handling: since v0.18.3 upstream loads them at run
    // time itself instead of linking import libraries for them.)
    if is_static && is_msvc {
        for sys in WINDOWS_SYSTEM_LIBS {
            println!("cargo:rustc-link-lib=dylib={sys}");
        }
    } else if is_static {
        emit_unix_static_deps(install_prefix.as_deref());
    }
}

/// Repeat, at the final link, the native dependencies upstream links PRIVATE
/// to a static `prism` on Unix-like targets.
///
/// A shared Prism resolves these inside `libprism.so`/`.dylib`; a static one
/// cannot, and because we consume the CMake build tree directly instead of
/// going through `find_package(prism)`, nothing else replays them. Without
/// this the final link fails on GLib/GIO symbols (Linux) or
/// `_AVSpeechUtteranceMaximumSpeechRate` & co. (Apple).
fn emit_unix_static_deps(install_prefix: Option<&Path>) {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Prism is C++23, and rustc drives the final link with a C driver, so the
    // C++ runtime has to be named explicitly.
    if APPLE_TARGET_OS.contains(&target_os.as_str()) {
        println!("cargo:rustc-link-lib=dylib=c++");
        // `cmake/PrismPlatformApple.cmake`. Frameworks are always present on
        // the SDK, and the power-management ones are behind a CMake option, so
        // naming the full set is simpler than mirroring the condition and
        // costs nothing: an unused framework adds no runtime dependency.
        for fw in apple_frameworks(&target_os) {
            println!("cargo:rustc-link-lib=framework={fw}");
        }
        return;
    }
    println!("cargo:rustc-link-lib=dylib=stdc++");

    // Everything else Prism needs on Unix arrives through pkg-config, and
    // which modules those are is decided at configure time (a backend whose
    // module is missing is skipped rather than fatal). So read the list the
    // build itself recorded rather than hardcoding one that would drift.
    let modules = pkgconfig_modules(install_prefix);
    if modules.is_empty() {
        println!(
            "cargo:warning=prism-sys: no pkg-config modules recorded for the static build; \
             if the final link fails on undefined backend symbols, that is why"
        );
    }
    for module in modules {
        // `probe` emits the link directives itself.
        if let Err(err) = pkg_config::Config::new().probe(&module) {
            println!(
                "cargo:warning=prism-sys: pkg-config could not resolve `{module}`, which the \
                 static Prism needs ({err}); the final link will likely fail"
            );
        }
    }
}

/// Target operating systems that use Apple frameworks rather than pkg-config.
const APPLE_TARGET_OS: &[&str] = &["macos", "ios", "tvos", "watchos", "visionos"];

/// The frameworks `cmake/PrismPlatformApple.cmake` links PRIVATE to `prism`.
/// Foundation, AVFoundation and the power-management pair are common to every
/// Apple platform; the UI framework differs per platform.
fn apple_frameworks(target_os: &str) -> Vec<&'static str> {
    let mut frameworks = vec!["Foundation", "AVFoundation", "IOKit", "CoreFoundation"];
    frameworks.push(match target_os {
        "macos" => "AppKit",
        "watchos" => "WatchKit",
        _ => "UIKit",
    });
    frameworks
}

/// The pkg-config modules the CMake build recorded as required.
///
/// For a static library upstream writes a `pkg_check_modules(... REQUIRED
/// IMPORTED_TARGET "<module>")` line into the generated `prism-config.cmake`
/// for every module it actually resolved (`PRISM_PKGCONFIG_FIND_DEPENDS` in
/// `cmake/PrismBackends.cmake` and `cmake/PrismPlatformUnix.cmake`). That file
/// is therefore an exact, per-build manifest of what the final link needs.
fn pkgconfig_modules(install_prefix: Option<&Path>) -> Vec<String> {
    let Some(prefix) = install_prefix else {
        return Vec::new();
    };
    let config = prefix.join("share/prism/prism-config.cmake");
    let Ok(text) = fs::read_to_string(&config) else {
        return Vec::new();
    };
    println!("cargo:rerun-if-changed={}", config.display());

    let mut modules = Vec::new();
    for line in text.lines() {
        if !line.trim_start().starts_with("pkg_check_modules(") {
            continue;
        }
        // ...REQUIRED IMPORTED_TARGET "giomm-2.68>=2.68.0")
        let Some(spec) = line.split('"').nth(1) else {
            continue;
        };
        // `probe` takes a bare module name; the version constraint was already
        // satisfied when CMake configured this build.
        let name = spec
            .split(['<', '>', '=', ' '])
            .next()
            .unwrap_or(spec)
            .trim();
        if !name.is_empty() && !modules.iter().any(|m| m == name) {
            modules.push(name.to_owned());
        }
    }
    modules
}

/// System libraries upstream links PRIVATE to `prism` on Windows
/// (`cmake/PrismPlatformWindows.cmake`), plus `oleaut32` for the BSTR/VARIANT
/// calls the JAWS, UIA and similar backends make. `powrprof` is behind a CMake
/// option upstream; naming it unconditionally is harmless.
const WINDOWS_SYSTEM_LIBS: &[&str] = &[
    "ole32",
    "oleaut32",
    "onecore",
    "runtimeobject",
    "uiautomationcore",
    "rpcrt4",
    "powrprof",
];
