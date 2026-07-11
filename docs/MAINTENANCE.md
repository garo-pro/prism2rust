# Maintenance & the self-maintaining model

This project is designed to keep itself current with upstream
[Prism](https://github.com/ethindp/prism) with minimal, well-scoped human (or
Claude) intervention. This document explains the moving parts and the exact
procedures. The procedures are also encoded as **skills** so they can be invoked
directly.

## The pin, restated

Everything hangs off one fact: the vendored `external/prism` submodule is pinned
to a **stable upstream release tag**. Three artifacts record it and must always
agree:

| Artifact | Role |
| --- | --- |
| Superproject gitlink (`git submodule status`) | The commit every clone checks out. Source of truth for reproducibility. |
| `PRISM_PIN.toml` | Machine-readable pin (tag, commit, license) the skills read/write. |
| `.gitmodules` header comment | Human-readable pin + policy. |

A CI/consistency check (or the `update-prism` skill) verifies these three match.

## Automated freshness

- **Dependabot** (`.github/dependabot.yml`) opens weekly PRs for the Cargo
  dependencies and the GitHub Actions. Those PRs are validated by CI like any
  other change.
- Dependabot's **git submodule** updater is intentionally *off*: it would move
  the pin to the latest branch commit, breaking the stable-tag policy. Upstream
  bumps are done deliberately via `update-prism`.
- **CI** (`.github/workflows/ci.yml`) gates every PR: format, clippy (`-D
  warnings`), pure-Rust unit tests, a native build + integration tests on
  Linux/macOS/Windows, and an MSRV check.

## Procedure: bump upstream Prism (`update-prism` skill)

1. Read the current pin from `PRISM_PIN.toml`.
2. `git -C external/prism fetch --tags` and list release tags. **Select the
   newest _stable_ tag** (skip pre-release/`-rcN`/date-suffixed tags).
3. `git -C external/prism checkout <tag>`; resolve the commit SHA.
4. Update `PRISM_PIN.toml` (`tag`, `commit`, `license`) and the `.gitmodules`
   header comment to match.
5. Stage the submodule gitlink so the new commit is recorded in the superproject.
6. Hand off to **`update-bridge`** to regenerate the FFI and reconcile API
   changes.
7. Only commit once `update-bridge` reports a green tree.

## Procedure: update the bridge (`update-bridge` skill)

The "bridge" is `crates/prism-sys` (raw FFI) plus the safe mappings in
`crates/prism` and the shared constants in `crates/prism-types`.

1. **Regenerate the raw FFI** from the newly-pinned header:
   ```bash
   LIBCLANG_PATH="<path to libclang>" \
   PRISM_SYS_NO_NATIVE=1 PRISM_SYS_UPDATE_PREGENERATED=1 \
   cargo build -p prism-sys --features bindgen
   ```
   This rewrites `crates/prism-sys/src/bindings_pregenerated.rs`.
2. **Review the FFI diff.** `git diff` on the generated file is the precise list
   of ABI changes (new/renamed/removed functions, struct fields, enum values,
   `PRISM_CONFIG_VERSION`).
3. **Reconcile `prism-types`** for any changed enums/ids/flags/config version.
   Each such constant has a test asserting its value against the header; update
   the constant *and* its test together.
4. **Reconcile the safe wrappers** (`crates/prism`) for new/changed/removed
   functions or vtable slots. Add wrapper methods for new capabilities and add
   `CustomBackend` trait methods + trampolines for new vtable entries.
5. **Add/extend tests** for everything touched (mandatory — see CLAUDE.md).
6. **Run the gate:**
   ```bash
   cargo fmt --all --check
   PRISM_SYS_NO_NATIVE=1 cargo clippy --workspace --all-targets -- -D warnings
   cargo test -p prism-types --all-features
   cargo test --workspace        # requires the C++23 toolchain
   ```
7. Commit the submodule bump, the regenerated bridge, the wrapper/test changes,
   and the pin files together.

## Why the crate version tracks upstream

The workspace `version` mirrors the pinned Prism release (e.g. `0.17.1`). This
makes the binding version self-documenting: a user can tell at a glance which
upstream API a given release wraps. `update-prism` bumps it as part of the pin.
