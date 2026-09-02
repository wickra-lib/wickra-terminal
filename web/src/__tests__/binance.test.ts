import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { backfill, binanceEvent, klinesAsEvents, openBinanceFeed, splitSymbol } from '../binance'
import type { FeedEvent, FeedState, StreamMessage } from '../binance'

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

  it('maps a ticker onto the quote and the turnover, as strings', () => {
    // The stream the bridge never subscribed to, so a browser watchlist could
    // not show a spread or a volume however well the core folded them.
    const message: StreamMessage = {
      stream: 'btcusdt@ticker',
      data: { c: '20010.5', b: '20009.1', a: '20011.9', v: '1234567.89' },
    }
    expect(binanceEvent(message, SYMBOL)).toEqual({
      type: 'ticker',
      symbol: SYMBOL,
      last: '20010.5',
      bid: '20009.1',
      ask: '20011.9',
      volume: '1234567.89',
    })
  })

  it('treats anything that is not a trade or a ticker as depth', () => {
    const message: StreamMessage = {
      stream: 'btcusdt@depth20@100ms',
      data: { lastUpdateId: 1, bids: [], asks: [] },
    }
    expect(binanceEvent(message, SYMBOL).type).toBe('book_snapshot')
  })
})

describe('splitSymbol', () => {
  it('splits BASE/QUOTE and refuses anything else', () => {
    expect(splitSymbol('ETH/USDT')).toEqual({ base: 'ETH', quote: 'USDT' })
    expect(() => splitSymbol('ETHUSDT')).toThrow(/BASE\/QUOTE/)
    expect(() => splitSymbol('/USDT')).toThrow(/BASE\/QUOTE/)
  })
})

describe('klinesAsEvents', () => {
  const row = (close: string, volume: string, closeTime: number) => [
    closeTime - 60_000,
    '1',
    '2',
    '0.5',
    close,
    volume,
    closeTime,
    'ignored',
  ]

  it('turns each bar into one print at its close', () => {
    // The bar's close time, not the moment the seed is fetched: the bars the
    // core rebuilds have to land where the venue's did rather than all inside
    // the bar that is open now.
    const events = klinesAsEvents([row('100', '5', 1_000), row('101', '6', 61_000)], SYMBOL)
    expect(events).toHaveLength(2)
    expect(events[0]).toEqual({
      type: 'trade',
      symbol: SYMBOL,
      price: '100',
      quantity: '5',
      aggressor: 'Buy',
      timestamp: 1_000,
    })
    expect(events[1].timestamp).toBe(61_000)
  })

  it('carries the venue volume rather than inventing one', () => {
    // A seed of size zero rebuilds bars the venue never printed, and every
    // volume indicator warmed on them reads as a flat market.
    expect(klinesAsEvents([row('100', '12.5', 1_000)], SYMBOL)[0].quantity).toBe('12.5')
  })

  it('drops a row that is not a kline instead of feeding a hole', () => {
    expect(klinesAsEvents([null, 'nope', [1, 2], row('1', '1', 1)], SYMBOL)).toHaveLength(1)
    expect(klinesAsEvents([[0, '1', '2', '0', '3', '4', 'later']], SYMBOL)).toHaveLength(0)
  })
})

describe('backfill', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('asks the venue for the pair and hands back the bars', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => [[0, '1', '2', '0.5', '100', '5', 1_000]],
    })
    vi.stubGlobal('fetch', fetchMock)

    const events = await backfill('ETH/USDT', '5m', 10)
    expect(events).toHaveLength(1)
    const url = String(fetchMock.mock.calls[0][0])
    expect(url).toContain('symbol=ETHUSDT')
    expect(url).toContain('interval=5m')
    expect(url).toContain('limit=10')
  })

  it('is best effort: a refusal or a throw means no history, not no market', async () => {
    // Exactly what the native source's backfill promises. A venue that does not
    // carry the interval, a request that times out and a market too new to have
    // a history all mean a terminal that starts empty.
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, json: async () => [] }))
    expect(await backfill('ETH/USDT')).toEqual([])

    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')))
    expect(await backfill('ETH/USDT')).toEqual([])
  })
})

/** A WebSocket stand-in that records what was done to it. */
class FakeSocket {
  static opened: FakeSocket[] = []
  onopen: (() => void) | null = null
  onmessage: ((event: MessageEvent<string>) => void) | null = null
  onerror: (() => void) | null = null
  onclose: (() => void) | null = null
  closed = false

  constructor(readonly url: string) {
    FakeSocket.opened.push(this)
  }

  close(): void {
    this.closed = true
    this.onclose?.()
  }
}

describe('openBinanceFeed', () => {
  beforeEach(() => {
    FakeSocket.opened = []
    vi.stubGlobal('WebSocket', FakeSocket)
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  it('subscribes trades, depth and the ticker', () => {
    const handle = openBinanceFeed('BTC/USDT', () => {})
    const { url } = FakeSocket.opened[0]
    expect(url).toContain('btcusdt@trade')
    expect(url).toContain('btcusdt@depth20@100ms')
    expect(url).toContain('btcusdt@ticker')
    handle.close()
  })

  it('drops a malformed frame instead of throwing out of the handler', () => {
    const seen: FeedEvent[] = []
    const handle = openBinanceFeed('BTC/USDT', (event) => seen.push(event))
    const socket = FakeSocket.opened[0]

    expect(() => socket.onmessage?.({ data: 'not json' } as MessageEvent<string>)).not.toThrow()
    expect(() => socket.onmessage?.({ data: '{}' } as MessageEvent<string>)).not.toThrow()
    expect(seen).toHaveLength(0)

    socket.onmessage?.({
      data: JSON.stringify({
        stream: 'btcusdt@trade',
        data: { p: '1', q: '1', m: false, T: 1 },
      }),
    } as MessageEvent<string>)
    expect(seen).toHaveLength(1)
    handle.close()
  })

  it('reconnects on a drop, doubling the wait and capping it', () => {
    // The socket used to carry `onmessage` and nothing else, so a dropped
    // connection left the chart frozen at the last print with nothing said.
    const states: FeedState[] = []
    const handle = openBinanceFeed('BTC/USDT', () => {}, (state) => states.push(state))
    FakeSocket.opened[0].onopen?.()
    expect(states).toEqual(['open'])

    FakeSocket.opened[0].onclose?.()
    expect(states).toEqual(['open', 'reconnecting'])
    expect(FakeSocket.opened).toHaveLength(1)

    // A quarter of a second, then half, then a full one: the native source's
    // schedule, so a blip recovers almost immediately.
    vi.advanceTimersByTime(250)
    expect(FakeSocket.opened).toHaveLength(2)

    FakeSocket.opened[1].onclose?.()
    vi.advanceTimersByTime(250)
    expect(FakeSocket.opened).toHaveLength(2)
    vi.advanceTimersByTime(250)
    expect(FakeSocket.opened).toHaveLength(3)

    handle.close()
  })

  it('resets the wait only once a socket is actually up', () => {
    // Resetting on the attempt turns the backoff into a fixed quarter-second
    // retry against an endpoint that accepts connections and drops them.
    const handle = openBinanceFeed('BTC/USDT', () => {})
    FakeSocket.opened[0].onclose?.()
    vi.advanceTimersByTime(250)
    FakeSocket.opened[1].onclose?.()
    vi.advanceTimersByTime(500)
    expect(FakeSocket.opened).toHaveLength(3)

    FakeSocket.opened[2].onopen?.()
    FakeSocket.opened[2].onclose?.()
    vi.advanceTimersByTime(250)
    expect(FakeSocket.opened).toHaveLength(4)
    handle.close()
  })

  it('an error closes the socket, which is what schedules the retry', () => {
    const handle = openBinanceFeed('BTC/USDT', () => {})
    FakeSocket.opened[0].onerror?.()
    expect(FakeSocket.opened[0].closed).toBe(true)
    vi.advanceTimersByTime(250)
    expect(FakeSocket.opened).toHaveLength(2)
    handle.close()
  })

  it('close stops reconnecting for good', () => {
    const states: FeedState[] = []
    const handle = openBinanceFeed('BTC/USDT', () => {}, (state) => states.push(state))
    handle.close()
    expect(states.at(-1)).toBe('closed')
    vi.advanceTimersByTime(60_000)
    expect(FakeSocket.opened).toHaveLength(1)
  })
})
