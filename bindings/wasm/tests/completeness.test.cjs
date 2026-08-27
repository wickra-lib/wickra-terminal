"use strict";

// Parity guard: the WASM binding must expose the full public surface, so an
// export dropped in a refactor fails loudly here rather than silently shipping a
// smaller reach to the browser. Mirrors the Node binding's completeness test.
//
// `scripts/check_binding_surface.py` asserts the same contract statically, from
// the C ABI header. This one asserts it against the artefact wasm-pack actually
// produced, which is where a `#[wasm_bindgen]` attribute lost in a refactor shows
// up — the static check reads the Rust source and would still see the function.

const { test } = require("node:test");
const assert = require("node:assert");
const wasm = require("../pkg-node/wickra_terminal_wasm.js");

test("module exposes Terminal and version", () => {
  assert.strictEqual(typeof wasm.Terminal, "function");
  assert.strictEqual(typeof wasm.version, "function");
});

test("Terminal exposes command and version", () => {
  for (const name of ["command", "version"]) {
    assert.strictEqual(
      typeof wasm.Terminal.prototype[name],
      "function",
      `Terminal is missing ${name}`,
    );
  }
});

test("Terminal exposes free, because the browser owns the handle", () => {
  // wasm-bindgen hands the caller a pointer with no finaliser: a Terminal that is
  // dropped without `free()` leaks the linear-memory allocation for the lifetime
  // of the page. The web renderer holds one for the session, but anything that
  // rebuilds a terminal — a config change, a source swap — must be able to
  // release the old one.
  assert.strictEqual(typeof wasm.Terminal.prototype.free, "function");
});
