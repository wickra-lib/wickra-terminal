# Wickra Terminal — WASM

WebAssembly bindings for the `wickra-terminal` data-driven core (wasm-bindgen).
This is the binding the web renderer runs on: build a `Terminal` from a JSON
config, drive it with command JSON, read back frame view-models — the same
protocol as the native TUI and every other binding.

The native exchange client cannot run in a browser sandbox, so the `live`
feature of the core is disabled here; the web renderer feeds a `Live` source over
the browser's own WebSocket.

## Build

```bash
wasm-pack build --target web
```

This produces a `pkg/` directory with the `.wasm` module and JS/TS glue.

## Usage (browser)

```js
import init, { Terminal, version } from "./pkg/wickra_terminal_wasm.js";

await init();

const term = new Terminal(JSON.stringify({
  sources: [{ Synth: { seed: 1 } }],
  layout: { panels: [{ kind: "Chart", rect: { x: 0, y: 0, w: 100, h: 100 } }] },
}));

term.command(JSON.stringify({ type: "Subscribe", source: 0, symbol: "BTC/USDT" }));
const frame = JSON.parse(term.command(JSON.stringify({ type: "Tick" })));
console.log(frame.panels[0], version());
```
