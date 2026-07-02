import type { ChartView } from '../types'

// Canvas renderer for the chart panel's view-model. The core produces the
// series and indicator values; this only draws them — the same view-model the
// TUI maps to a ratatui widget.
export function drawChart(canvas: HTMLCanvasElement, view: ChartView): void {
  const ctx = canvas.getContext('2d')
  if (!ctx) {
    return
  }
  const width = canvas.width
  const height = canvas.height

  ctx.fillStyle = '#0b0e14'
  ctx.fillRect(0, 0, width, height)

  const series = view.series
  if (series.length < 2) {
    return
  }

  let min = series[0]
  let max = series[0]
  for (const value of series) {
    if (value < min) {
      min = value
    }
    if (value > max) {
      max = value
    }
  }
  const range = max - min || 1
  const pad = 6

  ctx.strokeStyle = '#3b82f6'
  ctx.lineWidth = 1.5
  ctx.beginPath()
  series.forEach((value, index) => {
    const x = (index / (series.length - 1)) * width
    const y = height - ((value - min) / range) * (height - 2 * pad) - pad
    if (index === 0) {
      ctx.moveTo(x, y)
    } else {
      ctx.lineTo(x, y)
    }
  })
  ctx.stroke()
}
