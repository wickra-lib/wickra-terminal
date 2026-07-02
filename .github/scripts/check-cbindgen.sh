#!/usr/bin/env bash
# Fail if the committed C ABI header has drifted from the cbindgen output, so the
# hand-usable header can never fall out of sync with the Rust surface.
#
# Skips (exit 0) when cbindgen is not installed, so a checkout without the tool
# does not spuriously fail; CI installs cbindgen and runs the real check.
set -euo pipefail

header="bindings/c/include/wickra_terminal.h"

if ! command -v cbindgen >/dev/null 2>&1; then
    echo "cbindgen not installed — skipping C ABI header sync check."
    exit 0
fi

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

cbindgen --config bindings/c/cbindgen.toml --crate wickra-terminal-c --output "$tmp" --quiet

if ! diff -u "$header" "$tmp"; then
    echo ""
    echo "ERROR: $header is out of sync with cbindgen output."
    echo "Regenerate it with:"
    echo "  cbindgen --config bindings/c/cbindgen.toml --crate wickra-terminal-c --output $header"
    exit 1
fi

echo "C ABI header is in sync with cbindgen."
