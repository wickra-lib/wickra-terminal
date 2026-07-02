"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const { Terminal, version } = require("../index.js");

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
