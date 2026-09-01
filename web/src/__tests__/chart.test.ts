import { describe, expect, it } from 'vitest'

import { priceRange } from '../render/chart'
import type { OhlcBar } from '../types'

function bar(open: number, high: number, low: number, close: number): OhlcBar {
  return { open, high, low, close, volume: 1, timestamp: 0 }
}

describe('priceRange', () => {
  it('spans every wick, not just the bodies', () => {
    // A bar can trade far outside its body, and a scale taken from open and
    // close would clip the wicks off the top and bottom of the canvas.
    const range = priceRange([bar(10, 20, 5, 12), bar(12, 14, 1, 3)])
    expect(range).toEqual({ min: 1, range: 19 })
  })

  it('is null when there is nothing to draw', () => {
    // The renderer keys its fallback off this: no bars means the tick series is
    // all that exists, which is the first hour of an hourly session.
    expect(priceRange([])).toBeNull()
  })

  it('ignores a bar with no finite extent', () => {
    const range = priceRange([bar(10, Number.NaN, Number.NaN, 10), bar(10, 12, 8, 11)])
    expect(range).toEqual({ min: 8, range: 4 })
  })

  it('gives a market that has not moved some height', () => {
    // Zero range divides the whole plot by nothing; the candle must still draw.
    const range = priceRange([bar(100, 100, 100, 100)])
    expect(range!.range).toBeGreaterThan(0)
    expect(range!.min).toBeLessThan(100)
  })
})
