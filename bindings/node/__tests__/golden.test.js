"use strict";

// Cross-language golden parity, driven by `golden/manifest.json`.
//
// Each scenario names a config and a command sequence; replaying it must produce
// the frame in its expected file, byte for byte. Because the binding returns the
// core's compact `command_json` string verbatim, byte equality against that one
// file is the exact parity check.
//
// Reading the manifest rather than naming one scenario is what makes the corpus
// extensible: a scenario added in the Rust suite is picked up here, and in the
// seven other language suites, with no change to any of them.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const { Terminal } = require("../index.js");

function goldenDir() {
  let dir = __dirname;
  for (let i = 0; i < 8; i++) {
    const g = path.join(dir, "golden");
    if (fs.existsSync(path.join(g, "manifest.json"))) {
      return g;
    }
    dir = path.dirname(dir);
  }
  throw new Error("golden/ not found");
}

const golden = goldenDir();
const manifest = JSON.parse(fs.readFileSync(path.join(golden, "manifest.json"), "utf8"));

for (const scenario of manifest.scenarios) {
  test(`golden parity: ${scenario.name}`, () => {
    const config = fs.readFileSync(path.join(golden, scenario.config), "utf8");
    const expected = fs.readFileSync(path.join(golden, scenario.expected), "utf8").trim();
    const commands = fs
      .readFileSync(path.join(golden, scenario.commands), "utf8")
      .split("\n")
      .filter((line) => line.trim().length > 0);
    assert.ok(commands.length > 0, scenario.name);

    const term = new Terminal(config);
    let frame = "";
    for (const command of commands) {
      frame = term.command(command);
    }
    assert.strictEqual(frame.trim(), expected, scenario.name);
  });
}

test("the corpus covers more than one scenario", () => {
  // A manifest that silently shrank to one entry would leave every parity test
  // passing while checking a fraction of what it used to.
  const names = manifest.scenarios.map((s) => s.name);
  assert.ok(names.length >= 7, `only ${names.length} scenarios`);
  for (const expected of ["basic", "book_deltas", "footprint", "indicators", "seek"]) {
    assert.ok(names.includes(expected), `${expected} missing from the manifest`);
  }
});
