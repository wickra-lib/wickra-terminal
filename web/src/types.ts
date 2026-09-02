// The frame view-models, matching wickra-terminal-core's serde output. The PanelView
// enum is internally tagged by `panel`, with the variant's struct fields
// flattened alongside the tag.

export interface IndicatorField {
  name: string
  value: number
}

export interface IndicatorValue {
  name: string
  /** The primary value, or null while warming up. For a multi-output indicator
   *  this is its first field. */
  value: number | null
  /** Named outputs of a multi-output indicator, in declaration order. Absent
   *  entirely for single-output ones -- the core omits an empty list from the
   *  JSON, so this is optional rather than an empty array. */
  fields?: IndicatorField[]
  /** A bounded recent series, oldest first, ending at the current tick. Absent
   *  while the indicator is warming up. Indicators warm up at different lengths,
   *  so this is not always as long as the chart's own series; both end at the
   *  same tick, so a renderer aligns it to the right. */
  series?: number[]
}

/** One OHLCV bar of the configured timeframe. Distinct from `AltBar`, which the
 *  bars panel carries: an alternative bar has a direction and no time, because a
 *  Renko brick advances on price movement rather than on the clock. */
export interface OhlcBar {
  open: number
  high: number
  low: number
  close: number
  volume: number
  /** The bar's opening timestamp, ms since the Unix epoch. */
  timestamp: number
}

export interface ChartView {
  symbol: string
  last: number
  /** One point per trade, so finer than the bars and not waiting for a close. */
  series: number[]
  /** Closed bars, oldest first. Absent from the JSON until the first one
   *  closes, which at an hourly timeframe is the first hour of a session. */
  bars?: OhlcBar[]
  /** The bar still accumulating. Kept apart from `bars` because it is the one
   *  that will still change; a chart that omitted it would show the market
   *  frozen at the last close for a whole bar. */
  forming?: OhlcBar
  indicators: IndicatorValue[]
}

export interface Level {
  price: number
  quantity: number
}

export interface BookView {
  symbol: string
  bids: Level[]
  asks: Level[]
  spread: number | null
}

export interface TapePrint {
  price: number
  quantity: number
  side: string
  timestamp: number
}

export interface TapeView {
  symbol: string
  prints: TapePrint[]
}

export interface WatchRow {
  source: number
  symbol: string
  last: number
  bid: number
  ask: number
  volume: number
  /** Percent move from the first price the core folded for this market. */
  change: number
}

export interface WatchlistView {
  rows: WatchRow[]
}

export interface FootprintLevel {
  price: number
  buy: number
  sell: number
}

export interface FootprintView {
  symbol: string
  levels: FootprintLevel[]
}

/** One profile's histogram. `bins` is empty until the profile has produced one. */
export interface ProfileRow {
  label: string
  bins: number[]
  /** The lowest price the bins cover. Absent for a distribution over TIME --
   *  day of week, minute of session -- which has no price range at all. */
  price_low?: number
  price_high?: number
}

export interface ProfileView {
  symbol: string
  profiles: ProfileRow[]
}

/** One alternative bar. `direction` is +1 for a rising bar, -1 for a falling
 *  one; `volume` is absent for the bar types that do not carry it. */
export interface AltBar {
  open: number
  high: number
  low: number
  close: number
  direction: number
  volume?: number
}

export interface BarStreamView {
  label: string
  /** Most recent completed bars, oldest first. Empty until the stream completes
   *  one, which for a Renko brick or a point-and-figure column can take many
   *  candles: these charts advance on price movement rather than on time. */
  bars: AltBar[]
}

export interface BarsView {
  symbol: string
  streams: BarStreamView[]
}

export type PanelView =
  | ({ panel: 'chart' } & ChartView)
  | ({ panel: 'book' } & BookView)
  | ({ panel: 'tape' } & TapeView)
  | ({ panel: 'watchlist' } & WatchlistView)
  | ({ panel: 'footprint' } & FootprintView)
  | ({ panel: 'profile' } & ProfileView)
  | ({ panel: 'bars' } & BarsView)

export interface Frame {
  panels: PanelView[]
}

// The config side of the boundary. These mirror wickra-terminal-core's `RectSpec` and
// `PanelSpec`: the layout is data, and this renderer reads it rather than
// hard-coding a shape the config cannot change.

/** A panel rectangle in percent of the viewport. */
export interface RectSpec {
  x: number
  y: number
  w: number
  h: number
}

export type PanelKind =
  | 'Chart'
  | 'Book'
  | 'Tape'
  | 'Watchlist'
  | 'Footprint'
  | 'Profile'
  | 'Bars'

export interface PanelSpec {
  kind: PanelKind
  rect: RectSpec
}
