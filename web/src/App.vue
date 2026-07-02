<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Terminal } from 'wickra-terminal-wasm'
import type {
  BookView,
  ChartView,
  FootprintView,
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
        { kind: 'Book', rect: { x: 70, y: 0, w: 30, h: 35 } },
        { kind: 'Footprint', rect: { x: 70, y: 35, w: 30, h: 35 } },
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

// Runtime module toggle: add sources, subscribe/unsubscribe symbols live.
const sourceShorthand = ref('')
const subSource = ref(0)
const subSymbol = ref('ETH/USDT')
const status = ref('')

let terminal: Terminal | null = null
let timer: number | undefined
// The core assigns source ids sequentially; the config's source is 0.
let nextSourceId = 0

function parseSourceSpec(shorthand: string): Record<string, unknown> | null {
  const idx = shorthand.indexOf(':')
  if (idx < 0) {
    return null
  }
  const kind = shorthand.slice(0, idx)
  const rest = shorthand.slice(idx + 1)
  if (kind === 'synth') {
    const seedValue = Number(rest)
    return Number.isFinite(seedValue) ? { Synth: { seed: seedValue } } : null
  }
  if (kind === 'live') {
    const j = rest.indexOf(':')
    if (j < 0) {
      return null
    }
    return { Live: { venue: rest.slice(0, j), symbol: rest.slice(j + 1), testnet: false } }
  }
  if (kind === 'replay') {
    return { Replay: { dataset: rest } }
  }
  return null
}

function addSource(): void {
  if (!terminal) {
    return
  }
  const spec = parseSourceSpec(sourceShorthand.value)
  if (!spec) {
    status.value = 'bad source (synth:N | live:venue:SYM | replay:JSON)'
    return
  }
  terminal.command(JSON.stringify({ type: 'AddSource', spec }))
  const id = nextSourceId
  nextSourceId += 1
  const live = spec['Live'] as { symbol: string } | undefined
  if (live) {
    terminal.command(JSON.stringify({ type: 'Subscribe', source: id, symbol: live.symbol }))
  }
  status.value = `added source ${id}`
  sourceShorthand.value = ''
}

function subscribe(): void {
  if (!terminal) {
    return
  }
  terminal.command(
    JSON.stringify({ type: 'Subscribe', source: subSource.value, symbol: subSymbol.value }),
  )
  status.value = `subscribed ${subSymbol.value} on source ${subSource.value}`
}

function unsubscribe(source: number, sym: string): void {
  if (!terminal) {
    return
  }
  terminal.command(JSON.stringify({ type: 'Unsubscribe', source, symbol: sym }))
  status.value = `unsubscribed ${sym}`
}

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
const footprint = computed<FootprintView | undefined>(() => findPanel('footprint'))

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
  // The config opened one source (id 0); the next runtime source is id 1.
  nextSourceId = 1
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

    <div class="bar controls">
      <label>add source
        <input type="text" v-model="sourceShorthand" placeholder="synth:2 | live:binance:ETH/USDT" />
      </label>
      <button @click="addSource">add</button>
      <label>subscribe src <input type="number" v-model.number="subSource" min="0" /></label>
      <input type="text" v-model="subSymbol" />
      <button @click="subscribe">go</button>
      <span class="muted">{{ status }}</span>
    </div>

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

      <section class="panel footprint">
        <h2>Footprint {{ footprint?.symbol }}</h2>
        <table>
          <tr
            v-for="(lvl, i) in footprint?.levels ?? []"
            :key="i"
            :class="lvl.buy >= lvl.sell ? 'bid' : 'ask'"
          >
            <td>{{ lvl.price.toFixed(2) }}</td>
            <td>{{ lvl.buy.toFixed(3) }}</td>
            <td>×</td>
            <td>{{ lvl.sell.toFixed(3) }}</td>
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
            <td><button class="x" @click="unsubscribe(row.source, row.symbol)">×</button></td>
          </tr>
        </table>
      </section>
    </main>
  </div>
</template>
