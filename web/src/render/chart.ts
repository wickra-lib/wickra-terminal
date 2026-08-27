import type { ChartView } from '../types'

// Canvas renderer for the chart panel's view-model. The core produces the series
// and the indicator series; this only draws them — the same view-model the TUI
// maps to a ratatui widget.

/** Overlay colours, cycled per indicator. Deliberately not the price blue. */
const OVERLAY_COLOURS = ['#f59e0b', '#a78bfa', '#34d399', '#f472b6', '#22d3ee']

/** Vertical padding, so a line at the extreme is not clipped by the border. */
const PAD = 6

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
    const x = ((index + offset) / (total - 1)) * width
    const y = height - ((value - scale.min) / scale.range) * (height - 2 * PAD) - PAD
    if (index === 0) {
      ctx.moveTo(x, y)
    } else {
      ctx.lineTo(x, y)
    }
  })
  ctx.stroke()
}

export function drawChart(canvas: HTMLCanvasElement, view: ChartView): void {
  const ctx = canvas.getContext('2d')
  if (!ctx) {
    return
  }
  const width = canvas.width
  const height = canvas.height

  ctx.fillStyle = '#0b0e14'
  ctx.fillRect(0, 0, width, height)

  const price = view.series
  if (price.length < 2) {
    return
  }

  // Only overlay indicators whose values live on the price scale. A bounded
  // oscillator like RSI shares no range with the price, and forcing both onto
  // one axis would flatten the price line into a horizontal streak. Those keep
  // the numeric readout beside the chart instead.
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
      width,
      height,
      OVERLAY_COLOURS[index % OVERLAY_COLOURS.length],
      1,
    )
  })

  // The price last, so it is never hidden under an overlay.
  strokeSeries(ctx, price, total, scale, width, height, '#3b82f6', 1.5)
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
