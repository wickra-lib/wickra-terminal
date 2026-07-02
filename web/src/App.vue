<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Terminal } from 'wickra-terminal-wasm'
import type {
  BookView,
  ChartView,
  Frame,
  PanelView,
  TapeView,
  WatchlistView,
} from './types'
import { drawChart } from './render/chart'

const CONFIG_KEY = 'wickra-terminal-config'

function defaultConfig(seed: number): string {
  return JSON.stringify({
    sources: [{ Synth: { seed } }],
    layout: {
      panels: [
        { kind: 'Chart', rect: { x: 0, y: 0, w: 70, h: 70 } },
        { kind: 'Book', rect: { x: 70, y: 0, w: 30, h: 70 } },
        { kind: 'Tape', rect: { x: 70, y: 70, w: 30, h: 30 } },
        { kind: 'Watchlist', rect: { x: 0, y: 70, w: 70, h: 30 } },
      ],
    },
  })
}

const seed = ref(1)
const symbol = ref('BTC/USDT')
const frame = ref<Frame>({ panels: [] })
const chartCanvas = ref<HTMLCanvasElement | null>(null)

let terminal: Terminal | null = null
let timer: number | undefined

function findPanel<T extends PanelView['panel']>(
  name: T,
): Extract<PanelView, { panel: T }> | undefined {
  return frame.value.panels.find((p) => p.panel === name) as
    | Extract<PanelView, { panel: T }>
    | undefined
}

const chart = computed<ChartView | undefined>(() => findPanel('chart'))
const book = computed<BookView | undefined>(() => findPanel('book'))
const tape = computed<TapeView | undefined>(() => findPanel('tape'))
const watchlist = computed<WatchlistView | undefined>(() => findPanel('watchlist'))

function stop(): void {
  if (timer !== undefined) {
    clearInterval(timer)
    timer = undefined
  }
  if (terminal) {
    ;(terminal as { free?: () => void }).free?.()
    terminal = null
  }
}

function start(): void {
  stop()
  let cfg = localStorage.getItem(CONFIG_KEY)
  if (!cfg) {
    cfg = defaultConfig(seed.value)
    localStorage.setItem(CONFIG_KEY, cfg)
  }
  terminal = new Terminal(cfg)
  terminal.command(
    JSON.stringify({ type: 'Subscribe', source: 0, symbol: symbol.value }),
  )
  timer = window.setInterval(() => {
    if (!terminal) {
      return
    }
    frame.value = JSON.parse(terminal.command(JSON.stringify({ type: 'Tick' }))) as Frame
  }, 100)
}

function restart(): void {
  localStorage.setItem(CONFIG_KEY, defaultConfig(seed.value))
  start()
}

watch(frame, () => {
  const canvas = chartCanvas.value
  const view = chart.value
  if (canvas && view) {
    drawChart(canvas, view)
  }
})

onMounted(start)
onBeforeUnmount(stop)
</script>

<template>
  <div class="app">
    <header class="bar">
      <strong>Wickra Terminal</strong>
      <span class="muted">web renderer</span>
      <label>seed <input type="number" v-model.number="seed" min="0" /></label>
      <label>symbol <input type="text" v-model="symbol" /></label>
      <button @click="restart">restart</button>
    </header>

    <main class="grid">
      <section class="panel chart">
        <h2>Chart {{ chart?.symbol }} <span class="last">{{ chart?.last.toFixed(2) }}</span></h2>
        <canvas ref="chartCanvas" width="600" height="300"></canvas>
        <div class="indicators">
          <span v-for="ind in chart?.indicators ?? []" :key="ind.name">
            {{ ind.name }}={{ ind.value === null ? '…' : ind.value.toFixed(2) }}
          </span>
        </div>
      </section>

      <section class="panel book">
        <h2>Book</h2>
        <table>
          <tr v-for="(lvl, i) in (book?.asks ?? []).slice().reverse()" :key="'a' + i" class="ask">
            <td>{{ lvl.price.toFixed(2) }}</td><td>{{ lvl.quantity.toFixed(3) }}</td>
          </tr>
          <tr class="spread"><td colspan="2">spread {{ book?.spread?.toFixed(2) ?? '—' }}</td></tr>
          <tr v-for="(lvl, i) in book?.bids ?? []" :key="'b' + i" class="bid">
            <td>{{ lvl.price.toFixed(2) }}</td><td>{{ lvl.quantity.toFixed(3) }}</td>
          </tr>
        </table>
      </section>

      <section class="panel tape">
        <h2>Tape</h2>
        <table>
          <tr v-for="(pr, i) in tape?.prints ?? []" :key="i" :class="pr.side">
            <td>{{ pr.price.toFixed(2) }}</td><td>{{ pr.quantity.toFixed(3) }}</td><td>{{ pr.side }}</td>
          </tr>
        </table>
      </section>

      <section class="panel watchlist">
        <h2>Watchlist</h2>
        <table>
          <tr v-for="(row, i) in watchlist?.rows ?? []" :key="i">
            <td>[{{ row.source }}]</td><td>{{ row.symbol }}</td><td>{{ row.last.toFixed(2) }}</td>
          </tr>
        </table>
      </section>
    </main>
  </div>
</template>
