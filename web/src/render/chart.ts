import type { ChartView, OhlcBar } from '../types'

// Canvas renderer for the chart panel's view-model. The core produces the bars;
// this only draws them — the same view-model the TUI maps to a ratatui canvas.
//
// Candles rather than a line of last prices, because the view-model now carries
// the bars and a line of last trades is not what a chart of a market is. The
// tick series is still drawn when no bar has closed yet: at an hourly timeframe
// that is the first hour of every session, and an empty panel for an hour is
// worse than a rough line.
//
// Indicator overlays are drawn only in that fallback. An indicator's series is
// sampled once per tick while the candles are one per bar, so on the candle axis
// the two do not line up — an average would sit near, but not on, the bar it was
// computed from, which reads as a real reading and is not one.

/** Overlay colours, cycled per indicator. Deliberately not the price blue. */
const OVERLAY_COLOURS = ['#f59e0b', '#a78bfa', '#34d399', '#f472b6', '#22d3ee']

/** Vertical padding, so a line or a wick at the extreme is not clipped. */
const PAD = 6

/** Width of the price scale gutter, in pixels. */
const SCALE_WIDTH = 56

/** How many price labels the scale carries. */
const SCALE_ROWS = 5

const UP = '#34d399'
const DOWN = '#f87171'
const GROUND = '#0b0e14'
const MUTED = '#6b7280'

interface Scale {
  min: number
  range: number
}

/** The value range covering every series drawn, so they share one y-axis. */
function scaleOf(seriesList: number[][]): Scale {
  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const series of seriesList) {
    for (const value of series) {
      if (value < min) {
        min = value
      }
      if (value > max) {
        max = value
      }
    }
  }
  if (!Number.isFinite(min) || !Number.isFinite(max)) {
    return { min: 0, range: 1 }
  }
  return { min, range: max - min || 1 }
}

/**
 * The low and high across every bar drawn.
 *
 * A flat market has a zero range, which would divide the whole plot by nothing;
 * it gets a hair of height so the row still draws.
 */
export function priceRange(bars: OhlcBar[]): Scale | null {
  let low = Number.POSITIVE_INFINITY
  let high = Number.NEGATIVE_INFINITY
  for (const bar of bars) {
    if (Number.isFinite(bar.low)) {
      low = Math.min(low, bar.low)
    }
    if (Number.isFinite(bar.high)) {
      high = Math.max(high, bar.high)
    }
  }
  if (!Number.isFinite(low) || !Number.isFinite(high)) {
    return null
  }
  if (high === low) {
    const pad = Math.abs(high) * 0.0005 || 1
    return { min: low - pad, range: 2 * pad }
  }
  return { min: low, range: high - low }
}

/**
 * Draw one series across the full width, right-aligned.
 *
 * Right-aligned because every series ends at the current tick but they do not
 * all start there: an indicator with a long warmup has fewer points than the
 * price line. Aligning left would slide it backwards in time by exactly its
 * warmup, which looks like a lagging indicator and is not one.
 *
 * `total` is the longest series on the chart, so all of them share one x-axis.
 */
function strokeSeries(
  ctx: CanvasRenderingContext2D,
  series: number[],
  total: number,
  scale: Scale,
  left: number,
  width: number,
  height: number,
  colour: string,
  lineWidth: number,
): void {
  if (series.length < 2 || total < 2) {
    return
  }
  ctx.strokeStyle = colour
  ctx.lineWidth = lineWidth
  ctx.beginPath()
  const offset = total - series.length
  series.forEach((value, index) => {
    const x = left + ((index + offset) / (total - 1)) * width
    const y = height - ((value - scale.min) / scale.range) * (height - 2 * PAD) - PAD
    if (index === 0) {
      ctx.moveTo(x, y)
    } else {
      ctx.lineTo(x, y)
    }
  })
  ctx.stroke()
}

/** Draw the price scale down the left gutter. */
function drawScale(
  ctx: CanvasRenderingContext2D,
  scale: Scale,
  height: number,
): void {
  ctx.fillStyle = MUTED
  ctx.font = '10px ui-monospace, monospace'
  ctx.textAlign = 'right'
  ctx.textBaseline = 'middle'
  for (let row = 0; row < SCALE_ROWS; row += 1) {
    const fraction = row / (SCALE_ROWS - 1)
    const value = scale.min + scale.range * (1 - fraction)
    // Clamped inside the plot so the top and bottom labels are not half cut off
    // by the canvas edge.
    const y = Math.min(height - PAD, Math.max(PAD, PAD + fraction * (height - 2 * PAD)))
    ctx.fillText(value.toFixed(2), SCALE_WIDTH - 6, y)
  }
}

/** Draw one candle: the wick from low to high, the body from open to close. */
function drawCandle(
  ctx: CanvasRenderingContext2D,
  bar: OhlcBar,
  index: number,
  count: number,
  scale: Scale,
  left: number,
  width: number,
  height: number,
): void {
  const slot = width / count
  const centre = left + slot * (index + 0.5)
  // A gap between neighbours, and never wider than four pixels: a chart of six
  // bars should not draw six blocks the width of the panel.
  const body = Math.max(1, Math.min(slot * 0.64, 14))
  const y = (value: number): number =>
    height - ((value - scale.min) / scale.range) * (height - 2 * PAD) - PAD

  const rising = bar.close >= bar.open
  ctx.strokeStyle = rising ? UP : DOWN
  ctx.fillStyle = rising ? UP : DOWN

  ctx.lineWidth = 1
  ctx.beginPath()
  ctx.moveTo(centre, y(bar.high))
  ctx.lineTo(centre, y(bar.low))
  ctx.stroke()

  // A doji — open equal to close — has a body of no height, and a rect of no
  // height draws nothing at all. One pixel keeps the bar a trader most wants to
  // see on the chart.
  const top = y(Math.max(bar.open, bar.close))
  const bottom = y(Math.min(bar.open, bar.close))
  ctx.fillRect(centre - body / 2, top, body, Math.max(1, bottom - top))
}

export function drawChart(canvas: HTMLCanvasElement, view: ChartView): void {
  const ctx = canvas.getContext('2d')
  if (!ctx) {
    return
  }
  const width = canvas.width
  const height = canvas.height

  ctx.fillStyle = GROUND
  ctx.fillRect(0, 0, width, height)

  // Closed bars plus the one still forming: a chart that stopped at the last
  // close would show the market standing still for a whole bar.
  const bars = [...(view.bars ?? [])]
  if (view.forming) {
    bars.push(view.forming)
  }

  const range = priceRange(bars)
  if (range && bars.length > 0) {
    const plotLeft = SCALE_WIDTH
    const plotWidth = Math.max(1, width - SCALE_WIDTH)
    drawScale(ctx, range, height)
    bars.forEach((bar, index) => {
      drawCandle(ctx, bar, index, bars.length, range, plotLeft, plotWidth, height)
    })
    return
  }

  // No bar has closed yet. The tick series is all there is, and the overlays
  // share its axis, so this is the one place they can be drawn honestly.
  const price = view.series
  if (price.length < 2) {
    return
  }
  const overlays = (view.indicators ?? [])
    .map((indicator) => indicator.series ?? [])
    .filter((series) => series.length >= 2)
    .filter((series) => onPriceScale(series, price))

  const scale = scaleOf([price, ...overlays])
  const total = Math.max(price.length, ...overlays.map((s) => s.length))

  overlays.forEach((series, index) => {
    strokeSeries(
      ctx,
      series,
      total,
      scale,
      0,
      width,
      height,
      OVERLAY_COLOURS[index % OVERLAY_COLOURS.length],
      1,
    )
  })

  // The price last, so it is never hidden under an overlay.
  strokeSeries(ctx, price, total, scale, 0, width, height, '#3b82f6', 1.5)
}

/**
 * Whether a series sits close enough to the price to share its axis.
 *
 * A moving average tracks the price and belongs on the chart; an oscillator
 * bounded to 0..100 does not. The test is whether the series' own range
 * overlaps the price range at all — cheap, and it separates the two families
 * without the core having to declare which is which.
 */
function onPriceScale(series: number[], price: number[]): boolean {
  const s = scaleOf([series])
  const p = scaleOf([price])
  return s.min <= p.min + p.range && s.min + s.range >= p.min
}
