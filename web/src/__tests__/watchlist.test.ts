import { describe, expect, it } from 'vitest'
import { changeClass, compactVolume, spread } from '../watchlist'
import type { WatchRow } from '../types'

function row(over: Partial<WatchRow> = {}): WatchRow {
  return { source: 0, symbol: 'BTC/USDT', last: 0, bid: 0, ask: 0, volume: 0, change: 0, ...over }
}

describe('spread', () => {
  it('is the difference between the two sides of the quote', () => {
    expect(spread(row({ bid: 100, ask: 101.5 }))).toBe('1.50')
  })

  it('is a dash before the first ticker, not a zero', () => {
    // A market that has traded but never tickered. Reporting "0.00" would claim
    // a locked market where the terminal simply has no quote.
    expect(spread(row({ last: 100 }))).toBe('-')
  })

  it('is a dash when only one side has arrived', () => {
    expect(spread(row({ bid: 100 }))).toBe('-')
    expect(spread(row({ ask: 101 }))).toBe('-')
  })
})

describe('compactVolume', () => {
  it('abbreviates at each threshold and not below one', () => {
    expect(compactVolume(999.5)).toBe('999.50')
    expect(compactVolume(1_500)).toBe('1.50K')
    expect(compactVolume(2_500_000)).toBe('2.50M')
    expect(compactVolume(3_250_000_000)).toBe('3.25B')
  })

  it('keeps the sign, since a negative turnover is a feed fault worth seeing', () => {
    expect(compactVolume(-2_000_000)).toBe('-2.00M')
  })
})

describe('changeClass', () => {
  it('colours the movers and leaves an unmoved market plain', () => {
    expect(changeClass(1.2)).toBe('up')
    expect(changeClass(-1.2)).toBe('down')
    expect(changeClass(0)).toBe('flat')
  })
})
