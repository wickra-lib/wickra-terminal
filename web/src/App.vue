<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Terminal } from 'wickra-terminal-wasm'
import type {
  BarsView,
  BookView,
  ChartView,
  FootprintView,
  Frame,
  PanelSpec,
  PanelView,
  ProfileView,
  PanelKind,
  TapeView,
  WatchlistView,
} from './types'
import { placementsFor, readLayout, withLayout } from './layout'
import { binWidth, peakOf } from './profile'
import { parseIndicator } from './indicator'
import { actionFor, isTyping, readKeybinds, type Keybinds } from './keybinds'
import { reading } from './indicator'
import { parsePanel } from './panel'
import { changeClass, compactVolume, spread } from './watchlist'
import { drawChart } from './render/chart'
import { backfill, openBinanceFeed, type FeedEvent, type FeedState } from './binance'

const CONFIG_KEY = 'wickra-terminal-config'

/** How many catalogue names the search shows at once. */
const CATALOGUE_SHOWN = 40

/** How many steps one traversal of a recording takes. */
const SEEK_STEPS = 20

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
// Which market the keyboard acts on. The core owns focus; this mirrors it so a
// key can move it without a round trip to ask where it is.
const focusedSymbol = ref('BTC/USDT')
const frame = ref<Frame>({ panels: [] })
const chartCanvas = ref<HTMLCanvasElement | null>(null)

// Runtime module toggle: add sources, subscribe/unsubscribe symbols live.
const sourceShorthand = ref('')
const subSource = ref(0)
const subSymbol = ref('ETH/USDT')
const status = ref('')

// The registry, the timeframe and the time-machine. Five of the core's commands
// -- AddIndicator, RemoveIndicator, SetTimeframe, ListIndicators and Seek --
// reached neither renderer, so the indicator catalogue was settable only from a
// config file and a recording could not be scrubbed at all.
const indicatorShorthand = ref('Rsi 14')
const catalogueFilter = ref('')
const catalogue = ref<string[]>([])
const timeframe = ref('1m')
const replayCursor = ref(0)
const replayLength = ref(0)

// The keymap the config declares. It exists so both renderers share one, and
// until now only the TUI read it -- so rebinding a key moved half the product.
const keybinds = ref<Keybinds>({})
const indicatorField = ref<HTMLInputElement | null>(null)
const symbolField = ref<HTMLInputElement | null>(null)
const sourceField = ref<HTMLInputElement | null>(null)
const timeframeField = ref<HTMLInputElement | null>(null)
const catalogueField = ref<HTMLInputElement | null>(null)
const recordField = ref<HTMLInputElement | null>(null)
const panelField = ref<HTMLInputElement | null>(null)
/** The panel shorthand as typed: a kind, a rectangle, and an optional depth. */
const panelShorthand = ref('')
/** The recorder's capacity as typed, empty meaning stopped. */
const recordCapacity = ref('')
/** Whether this session is recording, so the export control can say so. */
const recording = ref(false)

let terminal: Terminal | null = null
let timer: number | undefined
// The core assigns source ids sequentially; the config's source is 0.
let nextSourceId = 0
// Cleanup functions for open browser-side exchange WebSocket bridges.
const feedBridges: Array<() => void> = []
/** What the browser's live bridge is doing, so the header can say which. */
const feedState = ref<FeedState>('closed')

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
  if (kind === 'replay') {
    return { Replay: { dataset: rest } }
  }
  return null
}

// `live:binance:BASE/QUOTE` — the WASM core cannot open sockets, so the browser
// opens the Binance stream itself and bridges it into a `Manual` source through
// the `Feed` command. Returns true if it handled the shorthand.
//
// Binance only. The native source reaches ten venues through `wickra-exchange`,
// and each speaks a stream dialect that would have to be written again here, so
// the limit is said out loud rather than discovered on the second venue anyone
// tries.
function addLiveBridge(shorthand: string): boolean {
  if (!terminal || !shorthand.startsWith('live:')) {
    return false
  }
  const rest = shorthand.slice('live:'.length)
  const j = rest.indexOf(':')
  const venue = j < 0 ? rest : rest.slice(0, j)
  const market = j < 0 ? '' : rest.slice(j + 1)
  if (venue !== 'binance' || !market) {
    status.value =
      'the browser feed is Binance spot only (live:binance:BASE/QUOTE); the native terminal reaches ten venues'
    return true
  }
  const id = nextSourceId
  nextSourceId += 1
  terminal.command(JSON.stringify({ type: 'AddSource', spec: 'Manual' }))
  terminal.command(JSON.stringify({ type: 'Subscribe', source: id, symbol: market }))

  /** Hand one event to the core, ignoring anything that arrives after teardown. */
  const push = (event: FeedEvent): void => {
    try {
      terminal?.command(JSON.stringify({ type: 'Feed', source: id, event }))
    } catch {
      /* terminal gone */
    }
  }

  try {
    const handle = openBinanceFeed(market, push, (state) => {
      feedState.value = state
      if (state === 'reconnecting') {
        // A dropped socket used to leave the chart frozen at the last print with
        // nothing said. A frozen chart that says it is frozen is a different
        // product from one that does not.
        status.value = `live binance ${market}: reconnecting`
      }
    })
    feedBridges.push(handle.close)
    status.value = `live binance ${market} on source ${id}`

    // History before the stream, the way a native subscription is seeded from
    // the venue's klines. Best effort and asynchronous: a market with no
    // history, or a REST call that fails, means a terminal that starts empty
    // rather than one that refuses to open the market. Late live prints
    // interleaving with the seed is harmless -- the fold is per event and the
    // bar builder is driven by timestamps, not by arrival order.
    void backfill(market).then((seed) => {
      for (const event of seed) {
        push(event)
      }
    })
  } catch (err) {
    status.value = `live failed: ${String(err)}`
  }
  sourceShorthand.value = ''
  return true
}

function addSource(): void {
  if (!terminal) {
    return
  }
  const shorthand = sourceShorthand.value.trim()
  if (addLiveBridge(shorthand)) {
    return
  }
  const spec = parseSourceSpec(shorthand)
  if (!spec) {
    status.value = 'bad source (synth:N | live:binance:BASE/QUOTE | replay:JSON)'
    return
  }
  terminal.command(JSON.stringify({ type: 'AddSource', spec }))
  const id = nextSourceId
  nextSourceId += 1
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

/** Apply a command and keep the frame the core answers with. */
function send(command: Record<string, unknown>): boolean {
  if (!terminal) {
    return false
  }
  try {
    const answer = terminal.command(JSON.stringify(command))
    // Every command answers with a frame except ListIndicators, which answers
    // with the catalogue; the caller for that one reads it instead.
    if (command.type !== 'ListIndicators') {
      frame.value = JSON.parse(answer) as Frame
    }
    status.value = `${String(command.type)} ok`
    return true
  } catch (err) {
    status.value = String(err)
    return false
  }
}

function addIndicator(): void {
  const spec = parseIndicator(indicatorShorthand.value)
  if (!spec) {
    status.value = 'bad indicator (Sma 20 | Macd 12 26 9 | Beta 20 vs ETH/USDT)'
    return
  }
  if (send({ type: 'AddIndicator', spec })) {
    status.value = `tracking ${spec.kind}`
  }
}

function removeIndicator(label: string): void {
  if (send({ type: 'RemoveIndicator', label })) {
    status.value = `removed ${label}`
  }
}

/** Drop a source and everything it owns.
 *
 * `RemoveSource` is in the boundary and bound in the TUI, and the browser had
 * no control for it: a source added here could be subscribed and unsubscribed
 * but never closed, so a mistyped venue stayed for the life of the tab.
 */
function removeSource(source: number): void {
  if (send({ type: 'RemoveSource', source })) {
    status.value = `removed source ${source}`
  }
}

/** Arm the recorder with a capacity, or stop it with an empty field.
 *
 * `SetRecording` is documented in all nine binding READMEs and was bound in
 * neither renderer, so the only way to record was a config field read once at
 * start-up -- which is the one thing nobody can reach when the market has just
 * done something worth keeping.
 */
function applyRecording(): void {
  const text = recordCapacity.value.trim()
  if (text === '') {
    if (send({ type: 'SetRecording', capacity: null })) {
      recording.value = false
      status.value = 'recording off'
    }
    return
  }
  const capacity = Number(text)
  if (!Number.isInteger(capacity) || capacity <= 0) {
    // Zero is not "off": it is a ring that drops everything it is handed, and a
    // control reporting "recording" against one would promise an empty file.
    status.value = 'capacity must be a whole number above zero, or empty to stop'
    return
  }
  if (send({ type: 'SetRecording', capacity })) {
    recording.value = true
    status.value = `recording, keeping ${capacity} events`
  }
}

/** Place a panel on the layout from the same shorthand the TUI prompt takes.
 *
 * The layout was read once from the config and never again, in both renderers.
 * The local copy is updated in step rather than re-read, because there is no
 * command that answers with the config -- and this renderer is the one issuing
 * the change, so it knows exactly what it did. Gated on the command being
 * accepted, so a refusal leaves the copy alone.
 */
function addPanel(): void {
  const spec = parsePanel(panelShorthand.value)
  if (!spec) {
    status.value = 'bad panel (Book 70 0 30 35 | Tape 0 70 100 30 48)'
    return
  }
  if (send({ type: 'AddPanel', spec })) {
    setLayout([...layout.value, spec])
    panelShorthand.value = ''
    status.value = `added ${spec.kind}`
  }
}

/** Take a panel off the layout, named by the kind its section draws.
 *
 * By kind rather than by index because that is what the browser has: the
 * placement map keeps the first spec per kind, so the section a person clicks
 * on is the first panel of that kind.
 */
function removePanel(kind: PanelKind): void {
  const index = layout.value.findIndex((spec) => spec.kind === kind)
  if (index < 0) {
    return
  }
  if (send({ type: 'RemovePanel', index })) {
    setLayout(layout.value.filter((_, at) => at !== index))
    status.value = `removed ${kind}`
  }
}

/** Hand the recorded events to the browser as the file `Replay` takes.
 *
 * Through `ExportRecording` rather than serialising here, for the same reason
 * the TUI does: the format is the core's to decide, and this way the file is
 * byte-for-byte what every other binding produces.
 */
function saveRecording(): void {
  if (!terminal) {
    return
  }
  let json: string
  try {
    json = terminal.command(JSON.stringify({ type: 'ExportRecording' }))
  } catch (err) {
    status.value = String(err)
    return
  }
  const events = JSON.parse(json) as unknown[]
  if (events.length === 0) {
    status.value = recording.value ? 'nothing recorded yet' : 'recording is off'
    return
  }
  const url = URL.createObjectURL(new Blob([json], { type: 'application/json' }))
  const link = document.createElement('a')
  link.href = url
  // Named by the wall clock rather than one path, because the thing a person
  // saves is a moment they want to keep and the next click must not take it
  // away.
  link.download = `wickra-recording-${Date.now()}.json`
  link.click()
  URL.revokeObjectURL(url)
  status.value = `wrote ${events.length} events`
}

function applyTimeframe(): void {
  if (send({ type: 'SetTimeframe', timeframe: timeframe.value })) {
    status.value = `timeframe ${timeframe.value}`
  }
}

/** Search the catalogue. It is five hundred entries, so a list is only useful
 *  once it is filtered — what a reader wants from it is the exact spelling of a
 *  name they half remember. */
function searchCatalogue(): void {
  if (!terminal) {
    return
  }
  try {
    const answer = JSON.parse(terminal.command(JSON.stringify({ type: 'ListIndicators' }))) as {
      indicators: Array<{ kind: string }>
    }
    const needle = catalogueFilter.value.trim().toLowerCase()
    catalogue.value = answer.indicators
      .map((entry) => entry.kind)
      .filter((kind) => needle === '' || kind.toLowerCase().includes(needle))
      .slice(0, CATALOGUE_SHOWN)
    status.value = `${catalogue.value.length} shown`
  } catch (err) {
    status.value = String(err)
  }
}

/** Scrub the replay source by a twentieth of its length. A recording is
 *  whatever length it is, and stepping one event at a time through fifty
 *  thousand of them is not scrubbing. */
function seekBy(direction: number): void {
  if (replayLength.value === 0) {
    status.value = 'the focused source is not replayable'
    return
  }
  const step = Math.max(1, Math.floor(replayLength.value / SEEK_STEPS))
  const target = Math.min(replayLength.value, Math.max(0, replayCursor.value + direction * step))
  if (send({ type: 'Seek', source: 0, index: target })) {
    refreshReplayPosition()
    status.value = `replay ${replayCursor.value}/${replayLength.value}`
  }
}

/** Where the config's source stands in its recording, or 0/0 if it is not one. */
function refreshReplayPosition(): void {
  if (!terminal) {
    return
  }
  try {
    const answer = JSON.parse(
      terminal.command(JSON.stringify({ type: 'ReplayPosition', source: 0 })),
    ) as { cursor: number; length: number }
    replayCursor.value = answer.cursor
    replayLength.value = answer.length
  } catch {
    replayLength.value = 0
  }
}

/** Move focus to the next or previous watched market. */
function cycleSymbol(forward: boolean): void {
  const rows = watchlist.value?.rows ?? []
  if (rows.length === 0) {
    status.value = 'nothing subscribed to focus'
    return
  }
  const at = rows.findIndex((row) => row.symbol === focusedSymbol.value)
  const next = (((at < 0 ? 0 : at) + (forward ? 1 : -1)) % rows.length + rows.length) % rows.length
  const row = rows[next]
  if (send({ type: 'SetFocus', source: row.source, symbol: row.symbol })) {
    focusedSymbol.value = row.symbol
    status.value = `focus ${row.symbol}`
  }
}

/**
 * Apply the action a key was bound to.
 *
 * The actions that open a prompt in the TUI focus the matching field here --
 * the same intent in the idiom each front-end has. `quit` is deliberately
 * unhandled: a browser tab is not the terminal's to close, and a page that
 * tried would be asking the user to confirm a dialog it had no business
 * raising.
 */
function runAction(action: string): boolean {
  switch (action) {
    case 'next_symbol':
      cycleSymbol(true)
      return true
    case 'prev_symbol':
      cycleSymbol(false)
      return true
    case 'seek_back':
      seekBy(-1)
      return true
    case 'seek_forward':
      seekBy(1)
      return true
    case 'list_indicators':
      catalogueField.value?.focus()
      return true
    case 'add_indicator':
      indicatorField.value?.focus()
      return true
    case 'remove_indicator': {
      const first = chart.value?.indicators[0]
      if (first) {
        removeIndicator(first.name)
      } else {
        status.value = 'no indicator to remove'
      }
      return true
    }
    case 'set_timeframe':
      timeframeField.value?.focus()
      return true
    case 'source_menu':
      sourceField.value?.focus()
      return true
    case 'add_symbol':
      symbolField.value?.focus()
      return true
    case 'set_recording':
      recordField.value?.focus()
      return true
    case 'add_panel':
      panelField.value?.focus()
      return true
    case 'save_recording':
      saveRecording()
      return true
    case 'remove_source': {
      // The source that owns the focused market, the way `remove_symbol` finds
      // the market itself: the watchlist row is where the two are paired.
      const row = (watchlist.value?.rows ?? []).find((r) => r.symbol === focusedSymbol.value)
      if (row) {
        removeSource(row.source)
      } else {
        status.value = 'no source to remove'
      }
      return true
    }
    case 'remove_symbol': {
      const row = (watchlist.value?.rows ?? []).find((r) => r.symbol === focusedSymbol.value)
      if (row) {
        unsubscribe(row.source, row.symbol)
      }
      return true
    }
    default:
      // quit and the panel-focus and scroll pairs: a browser tab is not ours to
      // close, and the panels are scrollable boxes the browser already drives.
      // `remove_panel` and `move_panel` go with the focus pair and for the same
      // reason -- with no focused panel here there is nothing for them to act
      // on, and a key that removed an arbitrary one would be worse than a key
      // that does nothing. Removing is the ✕ on the panel itself, which names
      // its target by being on it.
      // Reporting them as unhandled leaves the key to the browser.
      return false
  }
}

function onKeydown(event: KeyboardEvent): void {
  if (isTyping(event.target)) {
    return
  }
  const action = actionFor(event, keybinds.value)
  if (action !== null && runAction(action)) {
    event.preventDefault()
  }
}

function findPanel<T extends PanelView['panel']>(
  name: T,
): Extract<PanelView, { panel: T }> | undefined {
  return frame.value.panels.find((p) => p.panel === name) as
    | Extract<PanelView, { panel: T }>
    | undefined
}

// The layout the core was configured with. The TUI honours these rects; so does
// this renderer, which is the whole point of the panels being data rather than
// markup -- one config drives both front-ends, and a layout the config can
// express but a renderer cannot is a layout the terminal does not really have.
//
// The mapping itself lives in ./layout so it can be tested without mounting a
// component.
const layout = ref<PanelSpec[]>([])
const placements = computed(() => placementsFor(layout.value))

/** Hold the drawn layout and the stored config to the same set of panels.
 *
 * The config in `localStorage` is what the next reload starts from. Writing it
 * back here is what makes the panel controls worth using: a layout arranged
 * once survives, which is what `web/README.md` has always said happens.
 */
function setLayout(panels: PanelSpec[]): void {
  layout.value = panels
  const stored = localStorage.getItem(CONFIG_KEY)
  if (stored !== null) {
    localStorage.setItem(CONFIG_KEY, withLayout(stored, panels))
  }
}

const chart = computed<ChartView | undefined>(() => findPanel('chart'))
const book = computed<BookView | undefined>(() => findPanel('book'))
const tape = computed<TapeView | undefined>(() => findPanel('tape'))
const watchlist = computed<WatchlistView | undefined>(() => findPanel('watchlist'))
const footprint = computed<FootprintView | undefined>(() => findPanel('footprint'))
const profile = computed<ProfileView | undefined>(() => findPanel('profile'))
const bars = computed<BarsView | undefined>(() => findPanel('bars'))


function stop(): void {
  if (timer !== undefined) {
    clearInterval(timer)
    timer = undefined
  }
  while (feedBridges.length > 0) {
    feedBridges.pop()?.()
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
  layout.value = readLayout(cfg)
  keybinds.value = readKeybinds(cfg)
  terminal = new Terminal(cfg)
  terminal.command(
    JSON.stringify({ type: 'Subscribe', source: 0, symbol: symbol.value }),
  )
  // The config opened one source (id 0); the next runtime source is id 1.
  nextSourceId = 1
  refreshReplayPosition()
  timer = window.setInterval(() => {
    if (!terminal) {
      return
    }
    frame.value = JSON.parse(terminal.command(JSON.stringify({ type: 'Tick' }))) as Frame
    // A replay advances by itself as its events are consumed, so the scrubber
    // has to follow the source rather than only the seeks the user makes.
    refreshReplayPosition()
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

onMounted(() => {
  start()
  window.addEventListener('keydown', onKeydown)
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  stop()
})
</script>

<template>
  <div class="app">
    <header class="bar">
      <strong>Wickra Terminal</strong>
      <span class="muted">web renderer</span>
      <span
        v-if="feedState !== 'closed'"
        :class="feedState === 'open' ? 'muted' : 'reconnecting'"
      >{{ feedState === 'open' ? 'live' : 'reconnecting…' }}</span>
      <label>seed <input type="number" v-model.number="seed" min="0" /></label>
      <label>symbol <input type="text" v-model="symbol" /></label>
      <button @click="restart">restart</button>
    </header>

    <div class="bar controls">
      <label>add source
        <input
          ref="sourceField"
          type="text"
          v-model="sourceShorthand"
          placeholder="synth:2 | live:binance:ETH/USDT (Binance only) | replay:[…]"
        />
      </label>
      <button @click="addSource">add</button>
      <label>subscribe src <input type="number" v-model.number="subSource" min="0" /></label>
      <input ref="symbolField" type="text" v-model="subSymbol" />
      <button @click="subscribe">go</button>
      <span class="muted">{{ status }}</span>
    </div>

    <div class="bar controls">
      <label>indicator
        <input
          ref="indicatorField"
          type="text"
          v-model="indicatorShorthand"
          placeholder="Sma 20 | Beta 20 vs ETH/USDT"
        />
      </label>
      <button @click="addIndicator">add</button>

      <label>timeframe <input ref="timeframeField" type="text" v-model="timeframe" size="4" /></label>
      <button @click="applyTimeframe">set</button>

      <label>catalogue <input ref="catalogueField" type="text" v-model="catalogueFilter" placeholder="filter" /></label>
      <button @click="searchCatalogue">search</button>

      <!-- The time-machine. Disabled rather than hidden when the focused source
           cannot replay: a synthetic feed looks identical to a recording until
           you try to scrub it, and a control that silently does nothing is the
           worse of the two. -->
      <button @click="seekBy(-1)" :disabled="replayLength === 0" title="rewind">&#9664;</button>
      <span class="muted" v-if="replayLength > 0">{{ replayCursor }}/{{ replayLength }}</span>
      <span class="muted" v-else>not replayable</span>
      <button @click="seekBy(1)" :disabled="replayLength === 0" title="advance">&#9654;</button>

      <label>record
        <input
          ref="recordField"
          type="text"
          v-model="recordCapacity"
          size="6"
          placeholder="events"
        />
      </label>
      <button @click="applyRecording">{{ recording ? 'restart' : 'start' }}</button>
      <button @click="saveRecording">save</button>
      <span class="muted" v-if="recording">recording</span>

      <label>panel
        <input
          ref="panelField"
          type="text"
          v-model="panelShorthand"
          placeholder="Book 70 0 30 35"
        />
      </label>
      <button @click="addPanel">add</button>
    </div>

    <div class="bar catalogue" v-if="catalogue.length > 0">
      <button
        v-for="kind in catalogue"
        :key="kind"
        class="chip"
        @click="indicatorShorthand = kind"
      >{{ kind }}</button>
    </div>

    <main class="grid">
      <section class="panel chart" v-if="placements.Chart" :style="placements.Chart">
        <h2>Chart {{ chart?.symbol }} <span class="last">{{ chart?.last.toFixed(2) }}</span><button class="x drop" @click="removePanel('Chart')" title="take this panel off the layout">×</button></h2>
        <canvas ref="chartCanvas" width="600" height="300"></canvas>
        <div class="indicators">
          <span v-for="ind in chart?.indicators ?? []" :key="ind.name">
            {{ reading(ind) }}
            <button class="x" @click="removeIndicator(ind.name)" title="stop tracking">×</button>
          </span>
        </div>
      </section>

      <section class="panel book" v-if="placements.Book" :style="placements.Book">
        <h2>Book<button class="x drop" @click="removePanel('Book')" title="take this panel off the layout">×</button></h2>
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

      <section class="panel footprint" v-if="placements.Footprint" :style="placements.Footprint">
        <h2>Footprint {{ footprint?.symbol }}<button class="x drop" @click="removePanel('Footprint')" title="take this panel off the layout">×</button></h2>
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

      <section class="panel tape" v-if="placements.Tape" :style="placements.Tape">
        <h2>Tape<button class="x drop" @click="removePanel('Tape')" title="take this panel off the layout">×</button></h2>
        <table>
          <tr v-for="(pr, i) in tape?.prints ?? []" :key="i" :class="pr.side">
            <td>{{ pr.price.toFixed(2) }}</td><td>{{ pr.quantity.toFixed(3) }}</td><td>{{ pr.side }}</td>
          </tr>
        </table>
      </section>

      <section class="panel profile" v-if="placements.Profile" :style="placements.Profile">
        <h2>Profiles {{ profile?.symbol }}<button class="x drop" @click="removePanel('Profile')" title="take this panel off the layout">×</button></h2>
        <div v-for="row in profile?.profiles ?? []" :key="row.label" class="dist">
          <h3>
            {{ row.label }}
            <span v-if="row.price_low !== undefined && row.price_high !== undefined" class="muted">
              [{{ row.price_low.toFixed(2) }} – {{ row.price_high.toFixed(2) }}]
            </span>
          </h3>
          <p v-if="row.bins.length === 0" class="muted">warming up</p>
          <div v-else class="bins">
            <div
              v-for="(bin, i) in row.bins"
              :key="i"
              class="bin"
              :style="{ width: binWidth(bin, peakOf(row.bins)) }"
            ></div>
          </div>
        </div>
      </section>

      <section class="panel bars" v-if="placements.Bars" :style="placements.Bars">
        <h2>Bars {{ bars?.symbol }}<button class="x drop" @click="removePanel('Bars')" title="take this panel off the layout">×</button></h2>
        <div v-for="stream in bars?.streams ?? []" :key="stream.label" class="stream">
          <h3>{{ stream.label }}</h3>
          <p v-if="stream.bars.length === 0" class="muted">no bars completed yet</p>
          <template v-else>
            <!-- A mark per bar rather than a candle: these charts have no time
                 axis, so the sequence of ups and downs and the length of each
                 run is the whole shape. -->
            <div class="marks">
              <span
                v-for="(bar, i) in stream.bars"
                :key="i"
                :class="bar.direction >= 0 ? 'up' : 'down'"
              >{{ bar.direction >= 0 ? '▲' : '▼' }}</span>
            </div>
            <p class="muted last-bar">
              {{ stream.bars[stream.bars.length - 1].open.toFixed(2) }}
              &rarr; {{ stream.bars[stream.bars.length - 1].close.toFixed(2) }}
              [{{ stream.bars[stream.bars.length - 1].low.toFixed(2) }}
              {{ stream.bars[stream.bars.length - 1].high.toFixed(2) }}]
              <template v-if="stream.bars[stream.bars.length - 1].volume !== undefined">
                vol {{ stream.bars[stream.bars.length - 1].volume!.toFixed(3) }}
              </template>
            </p>
          </template>
        </div>
      </section>

      <section class="panel watchlist" v-if="placements.Watchlist" :style="placements.Watchlist">
        <h2>Watchlist<button class="x drop" @click="removePanel('Watchlist')" title="take this panel off the layout">×</button></h2>
        <table>
          <tr v-for="(row, i) in watchlist?.rows ?? []" :key="i">
            <td>[{{ row.source }}]</td><td>{{ row.symbol }}</td><td>{{ row.last.toFixed(2) }}</td>
            <td :class="changeClass(row.change)">{{ row.change.toFixed(2) }}%</td>
            <td>{{ spread(row) }}</td>
            <td>{{ compactVolume(row.volume) }}</td>
            <td><button class="x" @click="unsubscribe(row.source, row.symbol)" title="unsubscribe">×</button></td>
            <td><button class="x" @click="removeSource(row.source)" title="drop the source">⌫</button></td>
          </tr>
        </table>
      </section>
    </main>
  </div>
</template>
