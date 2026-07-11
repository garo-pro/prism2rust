# prism2rust

Rust bindings for **[Prism](https://github.com/ethindp/prism)** — the
Platform-agnostic Reader Interface for Speech and Messages, a single unified API
over screen readers (NVDA, JAWS, VoiceOver, Orca, …) and TTS engines (SAPI,
OneCore, Speech Dispatcher, …).

Upstream Prism is a C/C++23 library vendored as a git submodule and pinned to a
**stable release tag** (currently **v0.17.1** — see [`PRISM_PIN.toml`](PRISM_PIN.toml)).

## Crates

| Crate | What it is | Needs a C++ toolchain? |
| --- | --- | --- |
| [`prism-types`](crates/prism-types) | Pure-Rust value types: errors, backend ids, feature flags, log levels. | No |
| [`prism-sys`](crates/prism-sys) | Raw FFI. `build.rs` generates bindings (bindgen) and builds the vendored library (CMake). | Yes |
| [`prism`](crates/prism) | Safe, idiomatic wrapper: `Context`, `Backend`, `RegistryBuilder`, logging. | Yes |

## Quick start

```rust,no_run
use prism::Context;

fn main() -> prism::Result<()> {
    let ctx = Context::new()?;
    let mut backend = ctx.acquire_best()?;
    backend.speak("Hello from Rust", false)?;
    Ok(())
}
```

Register your own pure-Rust backend (this is how the test suite runs with no
real screen reader present):

```rust,no_run
use prism::{BackendFeatures, Context, CustomBackend, RegistryBuilder, Result};

struct Echo;
impl CustomBackend for Echo {
    fn speak(&mut self, text: &str, _interrupt: bool) -> Result<()> {
        println!("[echo] {text}");
        Ok(())
    }
}

let mut builder = RegistryBuilder::new()?;
builder.add_backend("echo", 100, BackendFeatures::SUPPORTS_SPEAK, || Echo)?;
let ctx = Context::builder().registry(builder.freeze()?).build()?;
# Ok::<(), prism::Error>(())
```

## Examples

Two runnable examples live in [`crates/prism/examples`](crates/prism/examples):

| Example | What it shows | Needs a real backend? |
| --- | --- | --- |
| `custom_backend` | Registers a pure-Rust `CustomBackend` and drives the full API surface. Runs anywhere. | No |
| `speak` | Enumerates backends, picks the best, and speaks — feature-gated and degrading gracefully. | Yes (uses your system TTS/screen reader) |

```bash
cargo run -p prism --example custom_backend   # portable, prints what it does
cargo run -p prism --example speak            # speaks aloud via the best backend
```

## Building

```bash
git clone <this repo>
cd prism2rust
git submodule update --init --recursive   # checks out the pinned Prism commit
cargo build --workspace                    # builds vendored Prism via CMake
cargo test --workspace
```

Requires a **C++23 compiler** and **CMake** for the native library. For a fast,
toolchain-free loop over the pure-Rust logic:

```bash
PRISM_SYS_NO_NATIVE=1 cargo clippy --workspace --all-targets -- -D warnings
cargo test -p prism-types --all-features
```

See [`crates/prism-sys/README.md`](crates/prism-sys/README.md) for build-script
knobs (`PRISM_LIB_DIR` to link a prebuilt library, etc.).

## Staying current

This repository is **self-maintaining**: Dependabot bumps crates/actions, CI
gates every change on Linux/macOS/Windows, and two skills handle upstream:

- **`update-prism`** — bump the submodule to the newest stable Prism tag (keeps
  the pin reproducible).
- **`update-bridge`** — regenerate the FFI and reconcile the wrappers with any
  API changes, with tests.

Details: [`docs/MAINTENANCE.md`](docs/MAINTENANCE.md) and [`CLAUDE.md`](CLAUDE.md).

## Testing policy

Everything is tested; no change lands without tests. Pure logic is covered by
`prism-types` unit tests (run anywhere); FFI and wrappers are covered by
integration tests driven through a custom backend against the real library. See
[`CLAUDE.md`](CLAUDE.md#testing-policy--mandatory).

## License

[MPL-2.0](LICENSE), matching upstream Prism. Third-party build/CI tooling
versions and their licenses are recorded in the workspace `Cargo.toml` and
`.github/workflows/ci.yml`.
