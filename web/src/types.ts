// The frame view-models, matching terminal-core's serde output. The PanelView
// enum is internally tagged by `panel`, with the variant's struct fields
// flattened alongside the tag.

export interface IndicatorValue {
  name: string
  value: number | null
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

export type PanelView =
  | ({ panel: 'chart' } & ChartView)
  | ({ panel: 'book' } & BookView)
  | ({ panel: 'tape' } & TapeView)
  | ({ panel: 'watchlist' } & WatchlistView)

export interface Frame {
  panels: PanelView[]
}
