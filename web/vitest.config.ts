import { defineConfig } from 'vitest/config'

// A configuration of its own rather than reusing vite.config.ts.
//
// The app config loads the WASM and top-level-await plugins, which exist to get
// a real WebAssembly module into the browser bundle. The tests here cover pure
// TypeScript — the layout mapping and the Binance message mapping — and pulling
// the whole plugin chain in to run them would only add ways for the test run to
// break for reasons that have nothing to do with the code under test.
//
// Anything that genuinely needs the core belongs in the WASM binding's own
// suite, which drives the real module through the same JSON boundary every other
// language uses.
export default defineConfig({
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
})
