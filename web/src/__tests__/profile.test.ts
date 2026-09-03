import { describe, expect, it } from 'vitest'

import { binWidth, peakOf } from '../profile'

describe('peakOf', () => {
  it('is the largest bin', () => {
    expect(peakOf([1, 7, 3])).toBe(7)
  })

  it('is zero for a profile that has produced nothing', () => {
    // The core sends an empty bin list until a profile has a histogram, which
    // is what the panel shows as "warming up" rather than as an empty chart.
    expect(peakOf([])).toBe(0)
  })

  it('ignores a bin that is not finite', () => {
    // A NaN peak would make every width NaN%, which the browser drops silently
    // and which then looks exactly like a profile with no data.
    expect(peakOf([2, Number.NaN, 5, Number.POSITIVE_INFINITY])).toBe(5)
  })
})

describe('binWidth', () => {
  it('scales a bin against the peak', () => {
    expect(binWidth(5, 10)).toBe('50%')
    expect(binWidth(10, 10)).toBe('100%')
  })

  it('draws nothing rather than something arbitrary', () => {
    // Each of these reached the renderer at some point in a profile's life: an
    // empty profile has no peak, a bin can legitimately be zero, and a
    // degenerate reading can be NaN. None of them is a bar of any length.
    expect(binWidth(1, 0)).toBe('0%')
    expect(binWidth(0, 10)).toBe('0%')
    expect(binWidth(Number.NaN, 10)).toBe('0%')
    expect(binWidth(-3, 10)).toBe('0%')
  })

  it('scales each profile against its own peak', () => {
    // The reason the peak is a parameter rather than computed across the panel:
    // volume traded at a price and a count of time slots are different units,
    // and one shared scale flattens whichever is smaller into nothing.
    const volume = [100, 250, 50]
    const timeOfDay = [2, 5, 1]
    expect(binWidth(volume[1], peakOf(volume))).toBe('100%')
    expect(binWidth(timeOfDay[1], peakOf(timeOfDay))).toBe('100%')
  })
})
