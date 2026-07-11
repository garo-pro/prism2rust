# prism

Safe, idiomatic Rust bindings for [Prism](https://github.com/ethindp/prism), the
Platform-agnostic Reader Interface for Speech and Messages — a single API over
screen readers (NVDA, JAWS, VoiceOver, Orca, …) and TTS engines (SAPI, OneCore,
Speech Dispatcher, …).

This crate wraps the raw [`prism-sys`](../prism-sys) FFI and is pinned to the
upstream release recorded in `PRISM_PIN.toml` at the repo root.

## Example

```no_run
use prism::Context;

fn main() -> prism::Result<()> {
    let ctx = Context::new()?;
    let mut backend = ctx.acquire_best()?;
    backend.speak("Hello from Rust", false)?;
    Ok(())
}
```

## Custom backends

Register your own pure-Rust backend and drive a `Context` against it — no real
screen reader required. This is how the crate's tests achieve deterministic,
cross-platform coverage:

```no_run
use prism::{Context, CustomBackend, RegistryBuilder, Result};
use prism::BackendFeatures;

struct Echo;
impl CustomBackend for Echo {
    fn speak(&mut self, text: &str, _interrupt: bool) -> Result<()> {
        println!("[echo] {text}");
        Ok(())
    }
}

let mut builder = RegistryBuilder::new()?;
builder.add_backend("echo", 100, BackendFeatures::SUPPORTS_SPEAK, || Echo)?;
let registry = builder.freeze()?;
let ctx = Context::builder().registry(registry).build()?;
# Ok::<(), prism::Error>(())
```

## Building

`prism-sys`'s `build.rs` compiles the vendored C/C++23 library via CMake, so a
C++23 compiler is required. See [`prism-sys`](../prism-sys) for the environment
variables that let you link a prebuilt library or skip the native build.

## License

MPL-2.0, matching upstream Prism.
