import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import wasm from 'vite-plugin-wasm'

// The terminal core is a real WebAssembly module (bindings/wasm, built with
// wasm-pack). The wasm plugin lets Vite bundle and instantiate it
// client-side, so the browser renderer runs the exact same core as the TUI.
// The module's async `init()` uses top-level await, which the esnext build
// target supports natively -- vite-plugin-top-level-await is not needed for
// that and does not run under Vite 8's Rolldown build.
export default defineConfig({
  base: '/',
  plugins: [vue(), wasm()],
  build: { target: 'esnext' },
})
