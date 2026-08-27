import { describe, expect, it } from 'vitest'

import { binanceEvent } from '../binance'
import type { StreamMessage } from '../binance'

const SYMBOL = { base: 'BTC', quote: 'USDT' }

describe('binanceEvent', () => {
  it('maps a trade onto the core event shape', () => {
    const message: StreamMessage = {
      stream: 'btcusdt@trade',
      data: { p: '20000.50', q: '0.125', m: false, T: 1_700_000_000_000 },
    }
    expect(binanceEvent(message, SYMBOL)).toEqual({
      type: 'trade',
      symbol: SYMBOL,
      price: '20000.50',
      quantity: '0.125',
      aggressor: 'Buy',
      timestamp: 1_700_000_000_000,
    })
  })

  it('inverts the maker flag to get the aggressor', () => {
    // Binance reports whether the *buyer* was the maker, so `m: true` means the
    // taker was the seller. Getting this backwards inverts every buy and sell in
    // the tape and the footprint, and nothing about the resulting chart looks
    // wrong — which is exactly why it is worth a test of its own.
    const maker: StreamMessage = {
      stream: 'btcusdt@trade',
      data: { p: '1', q: '1', m: true, T: 1 },
    }
    const taker: StreamMessage = {
      stream: 'btcusdt@trade',
      data: { p: '1', q: '1', m: false, T: 1 },
    }
    expect(binanceEvent(maker, SYMBOL).aggressor).toBe('Sell')
    expect(binanceEvent(taker, SYMBOL).aggressor).toBe('Buy')
  })

  it('keeps price and quantity as strings', () => {
    // The core parses them into exact decimals. Turning them into JavaScript
    // numbers on the way through would round the value before it ever reaches
    // the decimal type, which is the one thing this bridge must not do.
    const message: StreamMessage = {
      stream: 'btcusdt@trade',
      data: { p: '0.000000012345678', q: '123456789.123456789', m: false, T: 1 },
    }
    const event = binanceEvent(message, SYMBOL)
    expect(typeof event.price).toBe('string')
    expect(event.price).toBe('0.000000012345678')
    expect(event.quantity).toBe('123456789.123456789')
  })

  it('maps a depth message onto a book snapshot', () => {
    const message: StreamMessage = {
      stream: 'btcusdt@depth20@100ms',
      data: {
        lastUpdateId: 42,
        bids: [
          ['19999.00', '1.5'],
          ['19998.00', '2.5'],
        ],
        asks: [['20001.00', '1.2']],
      },
    }
    expect(binanceEvent(message, SYMBOL)).toEqual({
      type: 'book_snapshot',
      symbol: SYMBOL,
      last_update_id: 42,
      bids: [
        { price: '19999.00', quantity: '1.5' },
        { price: '19998.00', quantity: '2.5' },
      ],
      asks: [{ price: '20001.00', quantity: '1.2' }],
    })
  })

  it('treats anything that is not a trade stream as depth', () => {
    // The socket subscribes to exactly two streams, so the else branch is the
    // depth branch. This pins that, rather than leaving it to be inferred.
    const message: StreamMessage = {
      stream: 'btcusdt@depth20@100ms',
      data: { lastUpdateId: 1, bids: [], asks: [] },
    }
    expect(binanceEvent(message, SYMBOL).type).toBe('book_snapshot')
  })
})
