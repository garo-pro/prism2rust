// SPDX-License-Identifier: MPL-2.0
//
// Forward the delay-load list that `prism-sys` publishes (see its `build.rs`)
// into link arguments for this crate's own binaries, tests, examples and
// benches.
//
// It only ever has anything to do for a static MSVC build: a static Prism puts
// the screen-reader import libraries into *our* link, and those DLLs
// (`ZDSRAPI_x64.dll`, `PCTKUSR.dll`, ...) ship with the screen readers rather
// than with Windows. Without `/delayload` the executable hard-imports them and
// fails to start. Cargo cannot propagate link arguments through a dependency,
// so `prism-sys` publishes the names as `links` metadata and each consumer
// that links a binary repeats this step — downstream crates need the same few
// lines in their own build script (documented in `crates/prism-sys/README.md`).

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let Ok(dlls) = env::var("DEP_PRISM_DELAYLOAD") else {
        return;
    };

    // The unqualified form covers every linked artifact this package
    // produces -- integration tests, the lib's own unit-test binary, and
    // examples -- unlike the per-kind forms, which Cargo also rejects outright
    // for a kind the package does not have.
    for dll in dlls.split(';').filter(|d| !d.is_empty()) {
        println!("cargo:rustc-link-arg=/delayload:{dll}");
    }
    // Match upstream: unloading a delay-loaded module is allowed.
    println!("cargo:rustc-link-arg=/DELAY:unload");
    // Not every import library ends up referenced (the Orca and
    // speech-dispatcher bridges are Unix-only), and LNK4199 for an unused
    // /DELAYLOAD would otherwise fail the build under `-D warnings`.
    println!("cargo:rustc-link-arg=/ignore:4199");
}
