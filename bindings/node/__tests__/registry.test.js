"use strict";

// The indicator registry is reachable from Node.
//
// The registry lives in the Rust core and the binding passes JSON through, so
// nothing here needed new binding code. That is exactly why it is worth a test:
// "no code changed" is also what a broken pass-through looks like.

const { test } = require("node:test");
const assert = require("node:assert");
const { Terminal } = require("../index.js");

// A non-default indicator, so finding it proves the config reached the registry
// rather than the built-in overlay happening to look right.
const CONFIG = JSON.stringify({
  sources: [{ Synth: { seed: 1 } }],
  indicators: [{ kind: "Rsi", params: [14] }],
});

function chartIndicators(term) {
  const frame = JSON.parse(term.command(JSON.stringify({ type: "Tick" })));
  const chart = frame.panels.find((p) => p.panel === "chart");
  return chart.indicators.map((i) => i.name);
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
  const raw = term.command(JSON.stringify({ type: "ListIndicators" }));
  const { indicators } = JSON.parse(raw);
  assert.ok(indicators.length >= 495, `only ${indicators.length} entries`);
  // Every row carries the parameters needed to construct it. These are wickra's
  // reference values rather than the terminal's overlay: the catalogue answers
  // what this build can do, the overlay what it is showing.
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
  const chart = JSON.parse(raw).panels.find((p) => p.panel === "chart");
  const macd = chart.indicators[0];
  assert.strictEqual(macd.name, "MacdIndicator(12,26,9)");
  assert.ok(macd.fields.length > 1, JSON.stringify(macd));
  // The primary value is the first field, so a caller wanting one line does not
  // have to know which field that is.
  assert.strictEqual(macd.value, macd.fields[0].value);
});

test("a single-output indicator carries no fields key at all", () => {
  const term = subscribed();
  let raw = "";
  for (let i = 0; i < 30; i++) {
    raw = term.command(JSON.stringify({ type: "Tick" }));
  }
  const chart = JSON.parse(raw).panels.find((p) => p.panel === "chart");
  assert.ok(
    !Object.prototype.hasOwnProperty.call(chart.indicators[0], "fields"),
    "an empty field list must not appear on the wire",
  );
});
