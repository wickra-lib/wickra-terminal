import { createApp } from 'vue'
import init from 'wickra-terminal-wasm'
import App from './App.vue'
import './style.css'

// Instantiate the WebAssembly core before mounting, so the app can construct a
// Terminal synchronously. (Top-level await is enabled by vite-plugin-top-level-await.)
await init()

createApp(App).mount('#app')
