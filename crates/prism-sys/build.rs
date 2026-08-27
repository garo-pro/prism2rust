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
    let lib_dir: Option<PathBuf>;

    if let Some(dir) = env::var_os("PRISM_LIB_DIR") {
        // Use a prebuilt library.
        println!(
            "cargo:rustc-link-search=native={}",
            Path::new(&dir).display()
        );
        lib_dir = Some(PathBuf::from(&dir));
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
        lib_dir = Some(dst.join("lib"));
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
    // (and the generated screen-reader import libraries) has to be repeated at
    // the final link, because we consume the CMake build directly rather than
    // through `find_package(prism)`.
    if is_static && is_msvc {
        let import_libs = lib_dir
            .as_deref()
            .map(generated_import_libs)
            .unwrap_or_default();
        for imp in &import_libs {
            println!("cargo:rustc-link-lib=static={imp}");
        }
        for sys in WINDOWS_SYSTEM_LIBS {
            println!("cargo:rustc-link-lib=dylib={sys}");
        }
        emit_delayload_metadata(manifest_dir, &import_libs);
    }
}

/// System import libraries upstream links PRIVATE to `prism` on Windows
/// (`cmake/PrismPlatformWindows.cmake`), plus the COM/WinRT libraries the
/// plugin loader needs.
const WINDOWS_SYSTEM_LIBS: &[&str] = &[
    "delayimp",
    "onecore",
    "uiautomationcore",
    "rpcrt4",
    "powrprof",
    "ole32",
    "oleaut32",
];

/// Which of the screen-reader import libraries this build actually produced.
/// The set depends on the target architecture, so look for the ones we know
/// about rather than assuming, and never sweep up unrelated `.lib` files that
/// happen to sit in a `PRISM_LIB_DIR` we were pointed at.
fn generated_import_libs(lib_dir: &Path) -> Vec<String> {
    IMPORT_LIB_DEFS
        .iter()
        .map(|(name, ..)| *name)
        .filter(|name| lib_dir.join(format!("{name}.lib")).is_file())
        .map(str::to_owned)
        .collect()
}

/// Screen-reader import library -> the `defs/*.def` file CMake generates it
/// from (64-bit build, 32-bit build). Mirrors the table in upstream's
/// `cmake/PrismPlatformWindows.cmake`.
const IMPORT_LIB_DEFS: &[(&str, &str, &str)] = &[
    ("ZDSR", "zdsr.def", "zdsr32.def"),
    ("byctrl", "boy_pc_reader.def", "boy_pc_reader32.def"),
    ("PCTalker", "pc_talker.def", "pc_talker32.def"),
    (
        "PrismOrcaBridge",
        "prism_orca_bridge.def",
        "prism_orca_bridge32.def",
    ),
    (
        "PrismSpeechDispatcherBridge",
        "prism_speech_dispatcher_bridge.def",
        "prism_speech_dispatcher_bridge32.def",
    ),
];

/// Announce the screen-reader DLLs that the final binary must delay-load.
///
/// Those DLLs ship with the screen readers themselves and are absent on an
/// ordinary machine; upstream therefore links `prism.dll` with `/delayload:`
/// for each of them plus a failure hook (`source/delayimp.cpp`), so a missing
/// one degrades to "backend unavailable". When Prism is linked statically the
/// *consumer* performs that link, and without the flags the executable
/// hard-imports `ZDSRAPI_x64.dll` & co. and dies with STATUS_DLL_NOT_FOUND.
///
/// A build script cannot pass link arguments to a dependent crate's binary
/// (`cargo:rustc-link-arg` only reaches this package's own targets), and MSVC
/// rejects `/delayload` inside an object's `.drectve` section (LNK4229). So
/// publish the list as `links` metadata instead: every direct dependent sees
/// it as `DEP_PRISM_DELAYLOAD` and turns it into link arguments -- see
/// `crates/prism/build.rs`, which does exactly that for this workspace.
fn emit_delayload_metadata(manifest_dir: &Path, import_libs: &[String]) {
    let defs_dir = manifest_dir.join("../../external/prism/defs");
    let want_32bit = env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("x86");

    let mut dlls = Vec::new();
    for imp in import_libs {
        let Some((_, def64, def32)) = IMPORT_LIB_DEFS.iter().find(|(name, ..)| name == imp) else {
            continue;
        };
        let def = defs_dir.join(if want_32bit { def32 } else { def64 });
        println!("cargo:rerun-if-changed={}", def.display());
        match dll_name_from_def(&def) {
            Some(dll) => dlls.push(dll),
            None => println!(
                "cargo:warning=prism-sys: could not read the DLL name from {}; {imp} will be                  linked as an ordinary import and the binary will not start without that DLL",
                def.display()
            ),
        }
    }
    if dlls.is_empty() {
        return;
    }
    // For this crate's own linked artifacts (its unit-test binary)...
    for dll in &dlls {
        println!("cargo:rustc-link-arg=/delayload:{dll}");
    }
    println!("cargo:rustc-link-arg=/DELAY:unload");
    println!("cargo:rustc-link-arg=/ignore:4199");
    // ...and for every dependent, which has to repeat the step itself.
    println!("cargo:delayload={}", dlls.join(";"));
}

/// Read the `LIBRARY "name.dll"` statement out of a module-definition file.
fn dll_name_from_def(def: &Path) -> Option<String> {
    let text = fs::read_to_string(def).ok()?;
    text.lines()
        .find_map(|line| line.trim().strip_prefix("LIBRARY"))
        .map(|rest| rest.trim().trim_matches('"').to_owned())
        .filter(|name| !name.is_empty())
}
