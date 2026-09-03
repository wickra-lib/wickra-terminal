// A runnable Node example: rewind a recorded feed and watch state re-fold.
//
// The time-machine is what makes a recording more than a slow synthetic feed:
// `Seek` throws the folded state away and rebuilds it from the recording, so a
// rewind is deterministic rather than approximate. Nothing here is Node-specific
// -- it is four JSON commands, and every binding drives the same four.
//
//   cd bindings/node && npm run build
//   node examples/node/time_machine.js

const { Terminal } = require("wickra-terminal");

const PRICES = [100, 101, 102, 103, 104, 105];

function feed() {
  return JSON.stringify(
    PRICES.map((price, i) => ({
      type: "trade",
      symbol: { base: "BTC", quote: "USDT" },
      price: String(price),
      quantity: "1",
      aggressor: "Buy",
      timestamp: i + 1,
    })),
  );
}

const CONFIG = JSON.stringify({
  sources: [{ Replay: { dataset: feed() } }],
  layout: { panels: [{ kind: "Chart", rect: { x: 0, y: 0, w: 100, h: 100 } }] },
});

function chart(raw) {
  return JSON.parse(raw).panels.find((p) => p.panel === "chart");
}

function main() {
  const term = new Terminal(CONFIG);
  term.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));

  let raw = "";
  for (let i = 0; i < PRICES.length; i++) {
    raw = term.command(JSON.stringify({ type: "Tick" }));
  }
  console.log("played to the end:   last =", chart(raw).last);

  const where = JSON.parse(
    term.command(JSON.stringify({ type: "ReplayPosition", source: 0 })),
  );
  console.log(`position:            ${where.cursor}/${where.length}`);

  // Rewind to just after the second trade. The state is rebuilt from the
  // recording rather than restored from a snapshot, which is why a rewind lands
  // on exactly the frame the forward pass had at that point.
  raw = term.command(JSON.stringify({ type: "Seek", source: 0, index: 2 }));
  console.log("rewound to index 2:  last =", chart(raw).last);
  console.log("series:             ", chart(raw).series);

  // And forward again from there, over the same events.
  raw = term.command(JSON.stringify({ type: "Tick" }));
  console.log("one tick later:      last =", chart(raw).last);
}

main();
