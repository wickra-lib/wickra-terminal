import { describe, expect, it } from 'vitest'

import { parseIndicator } from '../indicator'

describe('parseIndicator', () => {
  it('reads a kind and its parameters', () => {
    expect(parseIndicator('Sma 20')).toEqual({ kind: 'Sma', params: [20] })
    expect(parseIndicator('Macd 12 26 9')).toEqual({ kind: 'Macd', params: [12, 26, 9] })
  })

  it('reads the label form the chart prints', () => {
    // So a name read off one renderer can be typed straight into the other, to
    // remove it or to add it back.
    expect(parseIndicator('Sma(20)')).toEqual({ kind: 'Sma', params: [20] })
    expect(parseIndicator('MacdIndicator(12,26,9)')).toEqual({
      kind: 'MacdIndicator',
      params: [12, 26, 9],
    })
  })

  it('splits the reference off before the parameters', () => {
    // A market has digits in it. Splitting on `vs` first is what keeps USDT's
    // pair from being read as a parameter.
    expect(parseIndicator('Beta 20 vs ETH/USDT')).toEqual({
      kind: 'Beta',
      params: [20],
      reference: 'ETH/USDT',
    })
    expect(parseIndicator('Beta(20) vs BTC/USDT')).toEqual({
      kind: 'Beta',
      params: [20],
      reference: 'BTC/USDT',
    })
  })

  it('omits the reference rather than sending an empty one', () => {
    // The core refuses a pairwise kind with no reference, and accepts one on a
    // kind that ignores it — so the difference between absent and empty matters.
    expect(parseIndicator('Sma 20')).not.toHaveProperty('reference')
  })

  it('takes a kind with no parameters', () => {
    expect(parseIndicator('AdaptiveCycle')).toEqual({ kind: 'AdaptiveCycle', params: [] })
  })

  it('returns null for what a user gets wrong', () => {
    expect(parseIndicator('')).toBeNull()
    expect(parseIndicator('   ')).toBeNull()
    expect(parseIndicator('Sma twenty')).toBeNull()
    expect(parseIndicator('Beta 20 vs ')).toBeNull()
  })
})
