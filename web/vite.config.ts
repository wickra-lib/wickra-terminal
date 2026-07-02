import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import wasm from 'vite-plugin-wasm'
import topLevelAwait from 'vite-plugin-top-level-await'

// The terminal core is a real WebAssembly module (bindings/wasm, built with
// wasm-pack). The two wasm plugins let Vite bundle and instantiate it
// client-side, so the browser renderer runs the exact same core as the TUI.
export default defineConfig({
  base: '/',
  plugins: [vue(), wasm(), topLevelAwait()],
  build: { target: 'esnext' },
})
