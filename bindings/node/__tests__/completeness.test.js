"use strict";

// Parity guard: the Node binding must expose the full public surface of the
// terminal, so an export dropped in a refactor fails loudly here (mirrors the
// completeness check in the main wickra repo).

const { test } = require("node:test");
const assert = require("node:assert");
const wickra = require("../index.js");

test("module exposes Terminal and version", () => {
  assert.strictEqual(typeof wickra.Terminal, "function");
  assert.strictEqual(typeof wickra.version, "function");
});

test("Terminal exposes command and version", () => {
  for (const name of ["command", "version"]) {
    assert.strictEqual(
      typeof wickra.Terminal.prototype[name],
      "function",
      `Terminal is missing ${name}`,
    );
  }
});
