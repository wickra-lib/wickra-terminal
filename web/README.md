# Wickra Terminal — Web renderer

The browser renderer of the Wickra trading terminal: a Vue 3 + Vite front-end
over the WASM binding (`bindings/wasm`). It runs the exact same `terminal-core`
as the native TUI, compiled to WebAssembly, and renders the identical frame
view-models — the chart to a canvas, the book/tape/watchlist to the DOM.

Web and TUI are two renderers of one core, not two products.

## Prerequisites

Build the WASM binding first (the web app depends on its `pkg/` via a `file:`
dependency):

```bash
( cd ../bindings/wasm && wasm-pack build --target web )
```

## Develop

```bash
npm install
npm run dev        # http://localhost:5173
```

## Build

```bash
npm run build      # vue-tsc typecheck + vite build -> dist/
npm run preview
```

## Notes

- The default source is the deterministic `Synth` feed; the layout is persisted
  in `localStorage`.
- A live source runs over the browser's own WebSocket (the native exchange
  client cannot run in a sandbox); real order execution needs a backend and is
  gated separately.
