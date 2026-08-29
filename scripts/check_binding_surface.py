#!/usr/bin/env python3
"""Assert that every binding exposes the surface the C ABI declares.

Ten language reaches sit on one C ABI. Each has its own test suite and each is
written separately, so a reach that falls behind fails nowhere: the golden corpus
compares *values*, and a binding that never grew a method simply has no test to
run. Nothing else in this repository holds the ten languages to one contract.

The header is the source of truth. Every export in it is a promise the bindings
make, so this reads `wickra_terminal_<name>` out of
`bindings/c/include/wickra_terminal.h` and checks each language's public surface
for that name, spelled the way that language spells it.

Unlike the backtester -- whose ABI maps to free functions -- this terminal's
reach is a handle type. `new` is a constructor, `command` an instance method and
`version` a static or free function, and each language spells those three
differently enough that a single pattern with a name-mangling rule would only
obscure what is being asserted. So each language declares its own matcher per
export, and the assertion stays the same: the name must be DECLARED, not merely
mentioned. Matching declarations matters -- a doc comment naming the function, or
an internal call site, would otherwise let a renamed export pass unnoticed.

Two exports are deliberately not required of every language:

  free_string   a memory-management detail of the ABI. Every binding frees the
                string it received; none of them exposes freeing as an API.
  free          handle lifetime, which each language expresses idiomatically:
                garbage collection in Python, Node and WASM, `IDisposable` in C#,
                `Close` in Go, `AutoCloseable` in Java, a registered finaliser in
                R. Demanding a literal `free` everywhere would be wrong, so it is
                reported where a language does expose an explicit disposal API
                and passed over where the runtime owns it.

Extras run the other way: a binding method with no export behind it is reported
as a note, not a failure. That is how a language gets *ahead* of the ABI, which is
worth seeing but is not drift in the dangerous direction.

Run from the repository root:  python scripts/check_binding_surface.py
"""

from __future__ import annotations

import glob
import os
import re
import sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
HEADER_REL = os.path.join("bindings", "c", "include", "wickra_terminal.h")
HEADER = os.path.join(ROOT, HEADER_REL)

# Exports that are ABI plumbing rather than a promise to callers.
ABI_ONLY = {"free_string"}
# Exports a language may express through its runtime instead of a named method.
OPTIONAL = {"free"}

# Where each language's public surface lives, and how it DECLARES each export.
# A missing key means the language does not spell that export at all, which is
# only allowed for the OPTIONAL set above.
BINDINGS = {
    # The pyclass carries the methods; __init__.py re-exports the class and the
    # version, so both files together are the surface a caller sees.
    "python": (
        ["bindings/python/src/lib.rs", "bindings/python/python/wickra_terminal/__init__.py"],
        {
            "new": r"#\[new\]",
            "command": r"fn command\s*\(",
            "version": r"#\[staticmethod\]\s*\n\s*fn version\s*\(|^__version__|\b__version__\b",
        },
    ),
    "node": (
        ["bindings/node/index.d.ts"],
        {
            "new": r"constructor\s*\(\s*configJson",
            "command": r"\bcommand\s*\(\s*cmdJson",
            "version": r"export declare function version\s*\(",
        },
    ),
    "wasm": (
        ["bindings/wasm/src/lib.rs"],
        {
            "new": r"#\[wasm_bindgen\(constructor\)\]",
            "command": r"pub fn command\s*\(",
            "version": r"pub fn version\s*\(",
        },
    ),
    # C++ consumes the same header as C, so it cannot drift in the exports it
    # CAN reach -- but the RAII wrapper can fall behind by not wrapping a new
    # one, which is invisible to a C-only check. `free` is the destructor:
    # that is how C++ expresses explicit disposal.
    "cpp": (
        ["bindings/c/include/wickra_terminal.hpp"],
        {
            "new": r"explicit Terminal[(]const std::string",
            "command": r"std::string command[(]const std::string",
            "version": r"inline std::string version[(][)]",
            "free": r"~Terminal[(][)]",
        },
    ),
    "csharp": (
        ["bindings/csharp/WickraTerminal/Terminal.cs"],
        {
            "new": r"public Terminal\s*\(\s*string",
            "command": r"public string Command\s*\(",
            "version": r"public static string Version\s*\(",
            "free": r"public void Dispose\s*\(",
        },
    ),
    "go": (
        ["bindings/go/wickra.go"],
        {
            "new": r"(?m)^func New\s*\(",
            "command": r"(?m)^func \(t \*Terminal\) Command\s*\(",
            "version": r"(?m)^func Version\s*\(",
            "free": r"(?m)^func \(t \*Terminal\) Close\s*\(",
        },
    ),
    "java": (
        ["bindings/java/src/main/java/org/wickra/terminal/Terminal.java"],
        {
            "new": r"public Terminal\s*\(\s*String",
            "command": r"public String command\s*\(",
            "version": r"public static String version\s*\(",
            "free": r"public void close\s*\(",
        },
    ),
    "r": (
        ["bindings/r/R/terminal.R"],
        {
            "new": r"(?m)^wkterm_new\s*<-\s*function",
            "command": r"(?m)^wkterm_command\s*<-\s*function",
            "version": r"(?m)^wkterm_version\s*<-\s*function",
        },
    ),
}

EXPORT = re.compile(r"\bwickra_terminal_([a-z0-9_]+)\s*\(")


def read(paths: list[str]) -> str:
    out = []
    for rel in paths:
        for path in sorted(glob.glob(os.path.join(ROOT, rel))):
            with open(path, encoding="utf-8") as handle:
                out.append(handle.read())
    return "\n".join(out)


def main() -> int:
    if not os.path.isfile(HEADER):
        print(f"header not found: {HEADER_REL}", file=sys.stderr)
        return 1
    with open(HEADER, encoding="utf-8") as handle:
        exports = sorted(set(EXPORT.findall(handle.read())))
    if not exports:
        print("no wickra_terminal_* exports found in the header", file=sys.stderr)
        return 1

    contract = [e for e in exports if e not in ABI_ONLY]
    required = [e for e in contract if e not in OPTIONAL]
    print(
        f"C ABI declares {len(exports)} exports; {len(required)} are required of "
        f"every binding ({', '.join(required)})."
    )

    failures, notes = [], []
    for lang, (paths, matchers) in BINDINGS.items():
        text = read(paths)
        if not text:
            failures.append(f"{lang}: no source found at {', '.join(paths)}")
            continue

        unknown = sorted(set(matchers) - set(contract))
        if unknown:
            failures.append(
                f"{lang}: matcher for {', '.join(unknown)}, which the header does not export"
            )

        missing = [
            e for e in required
            if e not in matchers or not re.search(matchers[e], text)
        ]
        present = [e for e in contract if e in matchers and re.search(matchers[e], text)]
        if missing:
            failures.append(f"{lang}: missing {', '.join(missing)}")
        drifted = "" if not missing else "  <-- DRIFTED"
        print(f"  {lang:<7} {len(present)}/{len(contract)} of the ABI surface{drifted}")

    # A binding that is ahead of the ABI is worth seeing, but it is not drift in
    # the direction that breaks callers.
    wasm = read(BINDINGS["wasm"][0])
    ahead = [
        m for m in re.findall(r"pub fn ([a-z_0-9]+)", wasm)
        if m not in contract and m not in {"instance_version"}
    ]
    if ahead:
        notes.append(
            "wasm exposes methods no export backs, so no other language can reach "
            f"them: {', '.join(sorted(set(ahead)))}"
        )

    for note in notes:
        print(f"\nnote: {note}")
    if failures:
        print("\nbinding surfaces disagree with the C ABI:", file=sys.stderr)
        for line in failures:
            print(f"  {line}", file=sys.stderr)
        return 1
    print("\nevery binding exposes the surface the C ABI declares.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
