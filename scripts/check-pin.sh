#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# Verify the Prism submodule pin is internally consistent: the commit recorded
# in the superproject's tree (the gitlink), in PRISM_PIN.toml, and in the
# .gitmodules header comment must all be identical. This is what guarantees
# every clone checks out the same, intended, stable release.
set -euo pipefail

cd "$(dirname "$0")/.."

fail() { echo "check-pin: $*" >&2; exit 1; }

# 1. Commit recorded as the gitlink in HEAD's tree (no submodule checkout needed).
gitlink="$(git ls-tree HEAD external/prism | awk '{print $3}')"
[ -n "$gitlink" ] || fail "could not read submodule gitlink for external/prism"

# 2. Commit declared in PRISM_PIN.toml.
pin_commit="$(grep -E '^commit[[:space:]]*=' PRISM_PIN.toml | head -1 | sed -E 's/.*"([0-9a-fA-F]+)".*/\1/')"
pin_tag="$(grep -E '^tag[[:space:]]*=' PRISM_PIN.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"
[ -n "$pin_commit" ] || fail "could not read commit from PRISM_PIN.toml"

# 3. Commit documented in the .gitmodules header comment.
gm_commit="$(grep -iE '^#[[:space:]]*Pinned commit' .gitmodules | head -1 | sed -E 's/.*: *([0-9a-fA-F]+).*/\1/')"
[ -n "$gm_commit" ] || fail "could not read 'Pinned commit' from .gitmodules"

echo "gitlink        : $gitlink"
echo "PRISM_PIN.toml : $pin_commit (tag $pin_tag)"
echo ".gitmodules    : $gm_commit"

[ "$gitlink" = "$pin_commit" ] || fail "gitlink != PRISM_PIN.toml commit"
[ "$gitlink" = "$gm_commit" ]  || fail "gitlink != .gitmodules commit"

# 4. Reject obvious pre-release tags (policy: pin stable releases only).
case "$pin_tag" in
  *-rc*|*-alpha*|*-beta*|*-pre*|*-preview*)
    fail "pinned tag '$pin_tag' looks like a pre-release; policy requires a stable tag" ;;
esac

echo "check-pin: OK — all three pin artifacts agree on $gitlink ($pin_tag)"
