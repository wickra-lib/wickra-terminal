"use strict";

// The indicator registry is reachable from WASM.
//
// This is the reach the web renderer runs on, so a registry that works
// everywhere except here would break the browser front-end and nothing else.
//
// Requires the nodejs target:
//   wasm-pack build bindings/wasm --target nodejs --out-dir pkg-node

const { test } = require("node:test");
const assert = require("node:assert");
const { Terminal } = require("../pkg-node/wickra_terminal_wasm.js");

// A non-default indicator, so finding it proves the config reached the registry
// rather than the built-in overlay happening to look right.
const CONFIG = JSON.stringify({
  sources: [{ Synth: { seed: 1 } }],
  indicators: [{ kind: "Rsi", params: [14] }],
});

function chartIndicators(term) {
  const frame = JSON.parse(term.command(JSON.stringify({ type: "Tick" })));
  return frame.panels.find((p) => p.panel === "chart").indicators.map((i) => i.name);
}

function subscribed() {
  const term = new Terminal(CONFIG);
  term.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));
  return term;
}

test("a configured indicator reaches the chart", () => {
  const term = subscribed();
  let names = [];
  for (let i = 0; i < 30; i++) {
    names = chartIndicators(term);
  }
  assert.deepStrictEqual(names, ["Rsi(14)"]);
});

test("indicators can be added and removed at run time", () => {
  const term = subscribed();
  term.command(
    JSON.stringify({ type: "AddIndicator", spec: { kind: "Atr", params: [14] } }),
  );
  assert.ok(chartIndicators(term).includes("Atr(14)"));
  term.command(JSON.stringify({ type: "RemoveIndicator", label: "Rsi(14)" }));
  assert.deepStrictEqual(chartIndicators(term), ["Atr(14)"]);
});

test("the catalogue lists the whole registry", () => {
  const term = subscribed();
  const { indicators } = JSON.parse(
    term.command(JSON.stringify({ type: "ListIndicators" })),
  );
  assert.ok(indicators.length >= 495, `only ${indicators.length} entries`);
  const byKind = Object.fromEntries(indicators.map((r) => [r.kind, r.params]));
  assert.strictEqual(byKind.Sma.length, 1);
  assert.strictEqual(byKind.MacdIndicator.length, 3);
  assert.deepStrictEqual(byKind.AdaptiveCycle, []);
});

test("an unknown indicator is rejected with its name", () => {
  const term = subscribed();
  assert.throws(
    () =>
      term.command(
        JSON.stringify({ type: "AddIndicator", spec: { kind: "NotReal" } }),
      ),
    /NotReal/,
  );
});

test("a multi-output indicator reports named fields", () => {
  const term = new Terminal(
    JSON.stringify({
      sources: [{ Synth: { seed: 1 } }],
      indicators: [{ kind: "MacdIndicator", params: [12, 26, 9] }],
    }),
  );
  term.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));
  let raw = "";
  for (let i = 0; i < 200; i++) {
    raw = term.command(JSON.stringify({ type: "Tick" }));
  }
  const macd = JSON.parse(raw).panels.find((p) => p.panel === "chart").indicators[0];
  assert.strictEqual(macd.name, "MacdIndicator(12,26,9)");
  assert.ok(macd.fields.length > 1, JSON.stringify(macd));
  assert.strictEqual(macd.value, macd.fields[0].value);
});
