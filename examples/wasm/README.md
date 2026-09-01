# Wickra Terminal WASM example

The shortest program that drives the terminal core from a browser: no build
tool, no framework, one HTML file that loads the module and issues commands.

The full Web renderer lives in [`web/`](../../web/) and is what you want if you
are looking for the product. This is what you want if you are looking for the
*shape* — how a page opens a terminal, subscribes a market and reads a frame.

## Build

The module ships as a `wasm-pack` `--target web` bundle. Build it once from the
repository root:

```bash
wasm-pack build bindings/wasm --target web --out-dir pkg
```

That drops `bindings/wasm/pkg/` with the `.wasm` binary, the JS loader and the
TypeScript types. The page imports the loader via
`../../bindings/wasm/pkg/wickra_terminal_wasm.js`.

## Serve

An ES-module script needs a real HTTP origin; `file://` will not load it. Any
static server from the repository root works:

```bash
python -m http.server 8000
# then open http://localhost:8000/examples/wasm/
```

## What it shows

A recorded feed rather than a synthetic one, deliberately: the page plays it to
the end, then rewinds to the second trade and shows the frame the forward pass
had at that point. That is the time-machine, and it is the thing a browser
cannot fake — `Seek` throws the folded state away and rebuilds it from the
recording, in WebAssembly, with no engine behind it.

The same three calls the native TUI makes: construct from a config, apply a
command, read the frame that comes back.
