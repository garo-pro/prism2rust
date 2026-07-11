---
name: update-prism
description: Bump the vendored upstream Prism submodule to the newest STABLE release tag, update the pin (PRISM_PIN.toml, .gitmodules, gitlink) so clones stay reproducible, then hand off to update-bridge to regenerate the FFI and reconcile API changes. Use when asked to update Prism, bump the submodule, or pull in a new upstream release.
---

# update-prism

Move the pinned upstream Prism to a newer **stable** release and keep every pin
artifact in sync. Do **not** edit the submodule commit by hand outside this flow.

## Guardrails

- **Only stable tags.** Skip anything pre-release: `-rc*`, `-alpha`, `-beta`,
  `-preview`, date/sha-suffixed tags like `v0.1.0-5a9457c`. If unsure whether a
  tag is stable, ask the user before proceeding.
- The three pin artifacts must end up identical: the superproject **gitlink**,
  `PRISM_PIN.toml`, and the `.gitmodules` header comment.
- Never commit until `update-bridge` reports a green tree.

## Steps

1. **Read the current pin.**
   ```bash
   cat PRISM_PIN.toml
   git submodule status external/prism
   ```

2. **Fetch and list candidate tags.**
   ```bash
   git -C external/prism fetch --tags --force
   git -C external/prism tag --list 'v*' | sort -V | tail -30
   ```
   Choose the newest tag that is a plain `vMAJOR.MINOR.PATCH` (no suffix). If the
   user named a specific version, use that (still verify it is stable).

3. **Confirm the target** with the user if it crosses a minor/major boundary
   (higher chance of API changes), otherwise proceed.

4. **Check out the tag and resolve the commit.**
   ```bash
   TAG=v0.X.Y
   git -C external/prism checkout "$TAG"
   git -C external/prism rev-parse HEAD   # -> COMMIT
   ```

5. **Update the pin artifacts** so all three agree:
   - `PRISM_PIN.toml`: set `tag`, `commit`, and (if changed) `license`.
   - `.gitmodules`: update the `Pinned release` / `Pinned commit` header lines.
   - Stage the submodule so the new gitlink is recorded:
     ```bash
     git add external/prism PRISM_PIN.toml .gitmodules
     ```

6. **Bump the workspace version** in the root `Cargo.toml`
   (`[workspace.package] version`) to match the new upstream release.

7. **Hand off to the bridge.** Invoke the `update-bridge` skill to regenerate the
   FFI, reconcile any API changes across `prism-types` and `prism`, add/adjust
   tests, and run the full gate.

8. **Commit** only after the bridge is green. Suggested message:
   ```
   deps: bump Prism to <TAG>

   Submodule external/prism -> <COMMIT>. Regenerated FFI bridge and
   reconciled API changes; all tests green.
   ```

## Verification

- `git submodule status external/prism` shows the new commit and `(<TAG>)`.
- `PRISM_PIN.toml`, `.gitmodules`, and the gitlink all reference the same commit.
- `update-bridge`'s gate passed.
