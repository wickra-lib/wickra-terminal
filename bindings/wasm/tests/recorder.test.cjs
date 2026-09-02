"use strict";

// The recorder, the scrubber and the host feed through the WASM binding,
// mirroring the Node suite so the two reaches are held to one expectation.
//
// Requires the nodejs target, which CI builds alongside the web one:
//   wasm-pack build bindings/wasm --target nodejs --out-dir pkg-node
//
// Four commands sit on the boundary, are documented in all nine binding
// READMEs, and were driven by almost no binding: `SetRecording` and
// `ExportRecording` by none at all, `ReplayPosition` only by the C example,
// `FeedDerivatives` by none. The README completeness test proved the promise
// and nothing checked it was kept, so the recorder had never been executed
// outside Rust.
//
// The round trip is the point: arm the recorder, drive the terminal, export
// what it kept, and hand that straight back as a `Replay` dataset. A binding
// that mangled the export would be caught by the replay refusing it, which no
// assertion about a string shape would find.

const { test } = require("node:test");
const assert = require("node:assert");
const { Terminal } = require("../pkg-node/wickra_terminal_wasm.js");

const SYMBOL = "BTC/USDT";

const CONFIG = JSON.stringify({
  sources: ["Manual"],
  // A derivatives indicator, so `FeedDerivatives` is observable in the frame
  // rather than merely accepted.
  indicators: [{ kind: "FundingRate", params: [] }],
  layout: { panels: [{ kind: "Chart", rect: { x: 0, y: 0, w: 100, h: 100 } }] },
});

function replayConfig(dataset) {
  return JSON.stringify({
    sources: [{ Replay: { dataset } }],
    indicators: [],
    layout: { panels: [{ kind: "Chart", rect: { x: 0, y: 0, w: 100, h: 100 } }] },
  });
}

function trade(price, timestamp) {
  return {
    type: "trade",
    symbol: { base: "BTC", quote: "USDT" },
    price,
    quantity: "0.5",
    aggressor: "Buy",
    timestamp,
  };
}

function chart(raw) {
  return JSON.parse(raw).panels.find((p) => p.panel === "chart");
}

function drive(terminal, price, timestamp) {
  terminal.command(JSON.stringify({ type: "Feed", source: 0, event: trade(price, timestamp) }));
  return terminal.command(JSON.stringify({ type: "Tick" }));
}

function subscribed() {
  const terminal = new Terminal(CONFIG);
  terminal.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: SYMBOL }));
  return terminal;
}

test("the recorder round trips through a replay", () => {
  const terminal = subscribed();

  // Nothing is kept until the recorder is armed, and asking is not an error.
  assert.deepStrictEqual(
    JSON.parse(terminal.command(JSON.stringify({ type: "ExportRecording" }))),
    [],
  );

  terminal.command(JSON.stringify({ type: "SetRecording", capacity: 64 }));
  ["100", "101", "102", "103"].forEach((price, i) => drive(terminal, price, i + 1));

  const exported = terminal.command(JSON.stringify({ type: "ExportRecording" }));
  const recorded = JSON.parse(exported);
  assert.strictEqual(recorded.length, 4);
  assert.strictEqual(recorded[0].price, "100");
  assert.strictEqual(recorded[3].price, "103");

  // Straight back in as a dataset: the shape `Replay` takes is the shape
  // `ExportRecording` answers with, which is what makes a session keepable.
  const replay = new Terminal(replayConfig(exported));
  replay.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: SYMBOL }));
  let raw;
  for (let i = 0; i < 4; i++) {
    raw = replay.command(JSON.stringify({ type: "Tick" }));
  }
  assert.strictEqual(chart(raw).last, 103);
});

test("stopping the recorder clears what it held", () => {
  // Both directions clear, so a capacity change never leaves a recording that
  // is part one size and part another.
  const terminal = subscribed();
  terminal.command(JSON.stringify({ type: "SetRecording", capacity: 64 }));
  drive(terminal, "100", 1);
  assert.strictEqual(
    JSON.parse(terminal.command(JSON.stringify({ type: "ExportRecording" }))).length,
    1,
  );

  terminal.command(JSON.stringify({ type: "SetRecording", capacity: null }));
  assert.deepStrictEqual(
    JSON.parse(terminal.command(JSON.stringify({ type: "ExportRecording" }))),
    [],
  );
});

test("replay position answers for a source that cannot be replayed", () => {
  // `0/0` rather than an error, so a renderer can ask about whatever is focused
  // without first knowing what kind of source it is.
  const terminal = subscribed();
  const where = JSON.parse(terminal.command(JSON.stringify({ type: "ReplayPosition", source: 0 })));
  assert.deepStrictEqual(where, { cursor: 0, length: 0 });
});

test("replay position tracks the cursor through a recording", () => {
  const terminal = subscribed();
  terminal.command(JSON.stringify({ type: "SetRecording", capacity: 64 }));
  ["100", "101", "102", "103"].forEach((price, i) => drive(terminal, price, i + 1));
  const exported = terminal.command(JSON.stringify({ type: "ExportRecording" }));

  const replay = new Terminal(replayConfig(exported));
  replay.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: SYMBOL }));
  assert.deepStrictEqual(
    JSON.parse(replay.command(JSON.stringify({ type: "ReplayPosition", source: 0 }))),
    { cursor: 0, length: 4 },
  );

  for (let i = 0; i < 3; i++) {
    replay.command(JSON.stringify({ type: "Tick" }));
  }
  assert.deepStrictEqual(
    JSON.parse(replay.command(JSON.stringify({ type: "ReplayPosition", source: 0 }))),
    { cursor: 3, length: 4 },
  );
});

test("fed derivatives reach a derivatives indicator", () => {
  // Accepting the command proves nothing on its own: the update is folded into
  // the market's microstructure and reaches an indicator only on the next
  // trade, so the reading is what says it arrived.
  const terminal = subscribed();
  assert.strictEqual(chart(drive(terminal, "100", 1)).indicators[0].value, null);

  terminal.command(
    JSON.stringify({
      type: "FeedDerivatives",
      source: 0,
      symbol: SYMBOL,
      update: {
        funding_rate: 0.0001,
        // All three prices, or the tick is withheld: a mark without an index
        // and a futures price is not a priced market.
        mark_price: 102,
        index_price: 100,
        futures_price: 104,
        open_interest: 1000,
        timestamp: 9,
      },
    }),
  );
  const reading = chart(drive(terminal, "101", 2)).indicators[0];
  assert.strictEqual(reading.name, "FundingRate");
  assert.ok(Math.abs(reading.value - 0.0001) < 1e-12, JSON.stringify(reading));
});

test("feeding derivatives to an untracked market is an error", () => {
  const terminal = new Terminal(CONFIG);
  assert.throws(() =>
    terminal.command(
      JSON.stringify({
        type: "FeedDerivatives",
        source: 0,
        symbol: SYMBOL,
        update: { funding_rate: 0.0001, timestamp: 1 },
      }),
    ),
  );
});
