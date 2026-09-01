"use strict";

// Streaming a feed and re-folding it in one batch reach the same frame.
//
// The terminal reaches a state two ways. Streaming folds one event per tick as
// it arrives; `Seek` throws the state away and re-folds the whole prefix in a
// single batch. ARCHITECTURE.md calls that re-fold the moat -- it is what makes
// a rewind deterministic and what lets the browser run the time-machine with no
// engine behind it -- so the two must land on byte-identical frames.
//
// Byte-identical, not merely equal: the binding returns the core's compact
// `command_json` string verbatim, so string equality here is the exact check
// with no JSON comparison in the way. The Rust suite proves the core re-folds
// correctly; this proves the binding carries the same bytes out.

const { test } = require("node:test");
const assert = require("node:assert");
const { Terminal } = require("../pkg-node/wickra_terminal_wasm.js");

const TICKS = 4;
const EVENTS = 8;

function feed() {
  const events = [];
  for (let i = 0; i < EVENTS; i++) {
    events.push({
      type: "trade",
      symbol: { base: "BTC", quote: "USDT" },
      price: String(100 + i),
      quantity: "1",
      aggressor: "Buy",
      timestamp: i + 1,
    });
  }
  return JSON.stringify(events);
}

function config() {
  return JSON.stringify({
    sources: [{ Replay: { dataset: feed() } }],
    layout: { panels: [{ kind: "Chart", rect: { x: 0, y: 0, w: 100, h: 100 } }] },
  });
}

function subscribed() {
  const term = new Terminal(config());
  term.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));
  return term;
}

test("streaming and a batch re-fold agree byte for byte", () => {
  const streamed = subscribed();
  let frame = "";
  for (let i = 0; i < TICKS; i++) {
    frame = streamed.command(JSON.stringify({ type: "Tick" }));
  }

  // A second terminal runs the feed out, then re-folds the same prefix in one
  // batch. Running past the point first is what makes this a rewind rather than
  // a replay of state it still had.
  const rewound = subscribed();
  for (let i = 0; i < EVENTS; i++) {
    rewound.command(JSON.stringify({ type: "Tick" }));
  }
  const refolded = rewound.command(
    JSON.stringify({ type: "Seek", source: 0, index: TICKS }),
  );

  assert.strictEqual(frame, refolded);
});

test("the frame compared is not an empty one", () => {
  // A guard on the guard: two empty frames are also byte-identical, and an
  // equality test that passes on nothing proves nothing.
  const term = subscribed();
  let raw = "";
  for (let i = 0; i < TICKS; i++) {
    raw = term.command(JSON.stringify({ type: "Tick" }));
  }
  const chart = JSON.parse(raw).panels.find((p) => p.panel === "chart");
  assert.strictEqual(chart.last, 100 + TICKS - 1);
});
