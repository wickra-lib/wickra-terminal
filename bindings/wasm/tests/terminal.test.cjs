"use strict";

// Behavioural suite for the WASM binding, mirroring the Node binding's
// terminal.test.js so the two reaches are held to the same expectations.
//
// Requires the nodejs target, which CI builds alongside the web one:
//   wasm-pack build bindings/wasm --target nodejs --out-dir pkg-node
// then:  node --test bindings/wasm/tests/*.test.cjs

const { test } = require("node:test");
const assert = require("node:assert");
const { Terminal, version } = require("../pkg-node/wickra_terminal_wasm.js");

const CONFIG = JSON.stringify({
  sources: [{ Synth: { seed: 1 } }],
  layout: {
    panels: [{ kind: "Chart", rect: { x: 0, y: 0, w: 100, h: 100 } }],
  },
});

test("version is a string and matches the instance method", () => {
  assert.strictEqual(typeof version(), "string");
  const t = new Terminal(CONFIG);
  assert.strictEqual(t.version(), version());
});

test("subscribe then tick returns a chart frame", () => {
  const t = new Terminal(CONFIG);
  t.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));
  let raw;
  for (let i = 0; i < 30; i++) {
    raw = t.command(JSON.stringify({ type: "Tick" }));
  }
  const frame = JSON.parse(raw);
  const charts = frame.panels.filter((p) => p.panel === "chart");
  assert.ok(charts.length > 0, "expected a chart panel");
  assert.ok(charts[0].last > 0);
});

test("invalid config throws", () => {
  assert.throws(() => new Terminal("not json"));
});

test("invalid command throws", () => {
  const t = new Terminal(CONFIG);
  assert.throws(() => t.command(JSON.stringify({ type: "Nope" })));
});

// The browser feeds a Live source over its own WebSocket, because the native
// exchange client cannot run in a sandbox — so Feed is the one command the web
// renderer depends on that the TUI never issues. It is worth its own case here.
test("a manual source accepts fed events", () => {
  const t = new Terminal(
    JSON.stringify({
      // `Manual` is a unit variant, so it serialises as a bare string, not a map.
      sources: ["Manual"],
      layout: {
        panels: [{ kind: "Tape", rect: { x: 0, y: 0, w: 100, h: 100 } }],
      },
    }),
  );
  t.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));
  // Event shape taken from golden/replay/basic.json, which is the serialised
  // form the core actually reads.
  t.command(
    JSON.stringify({
      type: "Feed",
      source: 0,
      event: {
        type: "trade",
        symbol: { base: "BTC", quote: "USDT" },
        price: "20000",
        quantity: "0.50",
        aggressor: "Buy",
        timestamp: 1,
      },
    }),
  );
  const frame = JSON.parse(t.command(JSON.stringify({ type: "Tick" })));
  const tape = frame.panels.find((p) => p.panel === "tape");
  assert.ok(tape, "expected a tape panel");
  assert.ok(tape.prints.length > 0, "expected the fed trade to reach the tape");
});
