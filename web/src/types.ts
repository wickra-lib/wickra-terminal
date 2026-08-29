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

export interface ChartView {
  symbol: string
  last: number
  series: number[]
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

export type PanelView =
  | ({ panel: 'chart' } & ChartView)
  | ({ panel: 'book' } & BookView)
  | ({ panel: 'tape' } & TapeView)
  | ({ panel: 'watchlist' } & WatchlistView)
  | ({ panel: 'footprint' } & FootprintView)

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

export type PanelKind = 'Chart' | 'Book' | 'Tape' | 'Watchlist' | 'Footprint'

export interface PanelSpec {
  kind: PanelKind
  rect: RectSpec
}
