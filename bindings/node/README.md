# Wickra Terminal — Node.js

Node.js bindings for the `wickra-terminal` data-driven core (napi-rs). Build a
`Terminal` from a JSON config, drive it with command JSON, read back frame
view-models — the same protocol as the native TUI and every other binding.

## Install

```bash
npm install wickra-terminal
```

## Usage

```js
const { Terminal, version } = require("wickra-terminal");

const term = new Terminal(JSON.stringify({
  sources: [{ Synth: { seed: 1 } }],
  layout: { panels: [{ kind: "Chart", rect: { x: 0, y: 0, w: 100, h: 100 } }] },
}));

term.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));
const frame = JSON.parse(term.command(JSON.stringify({ type: "Tick" })));
console.log(frame.panels[0]);
console.log(version());
```

## Build from source

```bash
npm install
npm run build   # regenerates index.js + index.d.ts (both committed) + the .node
npm test
```
