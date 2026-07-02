"use strict";

// Cross-language golden parity: build the terminal from the committed
// `golden/config.json`, replay the feed, and assert the frame equals
// `golden/expected/basic.min.json` byte-for-byte. Because the binding returns the
// core's compact `command_json` string verbatim, byte equality against that one
// file is the exact cross-language parity check.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const { Terminal } = require("../index.js");

function goldenDir() {
  let dir = __dirname;
  for (let i = 0; i < 8; i++) {
    const g = path.join(dir, "golden");
    if (fs.existsSync(path.join(g, "config.json"))) {
      return g;
    }
    dir = path.dirname(dir);
  }
  throw new Error("golden/ not found");
}

test("golden parity: config.json reproduces the byte-exact frame", () => {
  const g = goldenDir();
  const config = fs.readFileSync(path.join(g, "config.json"), "utf8");
  const expected = fs
    .readFileSync(path.join(g, "expected", "basic.min.json"), "utf8")
    .trim();

  const term = new Terminal(config);
  term.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));
  let frame = "";
  for (let i = 0; i < 32; i++) {
    frame = term.command(JSON.stringify({ type: "Tick" }));
  }
  assert.strictEqual(frame.trim(), expected);
});
