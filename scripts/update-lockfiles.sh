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
# If uv is not on PATH the script stops and tells you to install it
# (https://docs.astral.sh/uv/getting-started/installation/);
# WICKRA_BOOTSTRAP_UV=1 opts into fetching one pinned, checksum-verified release
# into a temporary directory instead.
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
# uv is not installed for you unless you ask. The previous version piped
# https://astral.sh/uv/install.sh straight into a shell, which runs whatever is
# behind that URL at that moment, with your privileges, on the machine of
# everyone who regenerates a lockfile. Set WICKRA_BOOTSTRAP_UV=1 to opt in; the
# bootstrap then fetches one pinned release archive and refuses to use it unless
# its checksum matches the one recorded here.
UV_VERSION="0.12.9"
uv_sha256() {
  case "$1" in
    x86_64-unknown-linux-gnu)  echo "ec7a99cd05e0cd7f80243f135ce1361c76835cb0ee60055d14d20eba8eba1460" ;;
    aarch64-unknown-linux-gnu) echo "c36fe17937ff6bd16dc42fc13854b5465999fcab2efe0af559381e945e3c6001" ;;
    aarch64-apple-darwin)      echo "301f72afaf54060f92da7016cb0115bd077f43a9c8e39c1d8170a0bac80fd398" ;;
    x86_64-apple-darwin)       echo "e1ca175824f1056589ce9908f7631879ebc3c36535b5e63dc06510beb370b4c1" ;;
    *)                         echo "" ;;
  esac
}

if ! command -v uv >/dev/null 2>&1; then
  if [ "${WICKRA_BOOTSTRAP_UV:-0}" != "1" ]; then
    echo "    uv is not on PATH." >&2
    echo "    Install it (https://docs.astral.sh/uv/getting-started/installation/)," >&2
    echo "    or re-run with WICKRA_BOOTSTRAP_UV=1 to fetch uv ${UV_VERSION} here." >&2
    exit 1
  fi

  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64)   uv_target="x86_64-unknown-linux-gnu" ;;
    Linux-aarch64)  uv_target="aarch64-unknown-linux-gnu" ;;
    Darwin-arm64)   uv_target="aarch64-apple-darwin" ;;
    Darwin-x86_64)  uv_target="x86_64-apple-darwin" ;;
    *)
      echo "    No pinned uv build for $(uname -s)-$(uname -m); install uv yourself." >&2
      exit 1
      ;;
  esac
  uv_expected="$(uv_sha256 "$uv_target")"

  echo "    bootstrapping uv ${UV_VERSION} (${uv_target})..."
  uv_dir="$(mktemp -d)"
  trap 'rm -rf "$uv_dir"' EXIT
  uv_archive="uv-${uv_target}.tar.gz"
  curl -fsSL --retry 5 --retry-all-errors -o "${uv_dir}/${uv_archive}"     "https://github.com/astral-sh/uv/releases/download/${UV_VERSION}/${uv_archive}"
  echo "${uv_expected}  ${uv_dir}/${uv_archive}" | sha256sum -c -
  tar -xzf "${uv_dir}/${uv_archive}" -C "$uv_dir" --strip-components=1
  export PATH="${uv_dir}:$PATH"
fi

req=".github/requirements"
cc="./scripts/update-lockfiles.sh"
uv pip compile --quiet --python-version 3.9  --generate-hashes --custom-compile-command "$cc" "$req/ci-dev-py39.in" -o "$req/ci-dev-py39.txt"
uv pip compile --quiet --python-version 3.10 --generate-hashes --custom-compile-command "$cc" "$req/ci-dev-py3.in"  -o "$req/ci-dev-py3.txt"

echo "==> Done. Review 'git diff' before committing."
