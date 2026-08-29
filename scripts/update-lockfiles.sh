#!/usr/bin/env bash
#
# Regenerate every committed lockfile in this repository:
#   - Rust:   Cargo.lock                          (cargo update)
#   - Node:   bindings/node, examples/node, web   (npm install --package-lock-only)
#   - Python: .github/requirements/*.txt          (uv pip compile --generate-hashes)
#
# Run from anywhere; the script finds the repository root itself:
#
#     ./scripts/update-lockfiles.sh
#
# fuzz/Cargo.lock is deliberately absent from that list: `fuzz/` is a detached
# crate whose lock cargo-fuzz ignores, and the smoke job resolves it fresh.
#
# The Python locks are hash-pinned, which is what the OpenSSF Scorecard
# PinnedDependencies check looks for and what lets CI install with
# `--require-hashes`. They are generated with uv rather than pip-tools because uv
# resolves a *target* Python version's full transitive closure, with hashes,
# without that interpreter being installed here. That matters: the tooling is
# locked twice, because pytest 9 carries the PYSEC-2026-1845 fix and needs
# Python >= 3.10, while 3.9 is the abi3 floor the wheel is built against. Each
# .in file says so at the top.
#
# If uv is not on PATH the script bootstraps a local copy on Linux and macOS. On
# Windows, install it first: https://docs.astral.sh/uv/getting-started/installation/
#
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "==> Rust (Cargo.lock)"
cargo update

echo "==> Node (bindings/node, examples/node, web)"
for dir in bindings/node examples/node web; do
  echo "    $dir"
  (cd "$dir" && npm install --package-lock-only --no-audit --no-fund)
done

echo "==> Python (.github/requirements/*.txt via uv)"
if ! command -v uv >/dev/null 2>&1; then
  echo "    uv not found on PATH; bootstrapping a local copy..."
  curl -LsSf https://astral.sh/uv/install.sh | sh
  export PATH="$HOME/.local/bin:$PATH"
fi

req=".github/requirements"
cc="./scripts/update-lockfiles.sh"
uv pip compile --quiet --python-version 3.9  --generate-hashes --custom-compile-command "$cc" "$req/ci-dev-py39.in" -o "$req/ci-dev-py39.txt"
uv pip compile --quiet --python-version 3.10 --generate-hashes --custom-compile-command "$cc" "$req/ci-dev-py3.in"  -o "$req/ci-dev-py3.txt"

echo "==> Done. Review 'git diff' before committing."
