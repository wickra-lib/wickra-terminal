// A runnable Node.js example: drive a synthetic feed and print a frame.
//
//   ( cd bindings/node && npm install && npm run build )
//   ( cd examples/node && npm install && node synth_terminal.js )

"use strict";

const { Terminal, version } = require("wickra-terminal");

const CONFIG = JSON.stringify({
  sources: [{ Synth: { seed: 1 } }],
  layout: { panels: [{ kind: "Chart", rect: { x: 0, y: 0, w: 100, h: 100 } }] },
});

const term = new Terminal(CONFIG);
term.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));

let raw = "";
for (let i = 0; i < 20; i++) {
  raw = term.command(JSON.stringify({ type: "Tick" }));
}

console.log("wickra-terminal", version());
console.log(JSON.stringify(JSON.parse(raw).panels[0], null, 2));
