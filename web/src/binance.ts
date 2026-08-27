// A browser-side bridge from a Binance market-data WebSocket into the terminal's
// data-driven boundary. The WASM core cannot open native sockets, so the browser
// opens the public stream, parses each message into the core's `Event` JSON, and
// pushes it in through the `Feed` command on a `Manual` source. Public market
// data only — no API keys, no orders.

export interface FeedEvent {
  type: 'trade' | 'book_snapshot'
  symbol: { base: string; quote: string }
  [field: string]: unknown
}

export interface BinanceTrade {
  p: string
  q: string
  m: boolean
  T: number
}

export interface BinanceDepth {
  lastUpdateId: number
  bids: [string, string][]
  asks: [string, string][]
}

export interface StreamMessage {
  stream: string
  data: BinanceTrade | BinanceDepth
}

function level(pair: [string, string]): { price: string; quantity: string } {
  return { price: pair[0], quantity: pair[1] }
}

/**
 * Map one Binance stream message onto the core's `Event` JSON.
 *
 * Pure, and separate from the socket, so the mapping can be tested without
 * opening one. That matters most for `aggressor`: Binance reports whether the
 * *buyer* was the maker, so a true flag means the taker was the seller. Getting
 * that backwards inverts every buy and sell in the tape and the footprint, and
 * nothing about the resulting chart looks wrong.
 */
export function binanceEvent(
  message: StreamMessage,
  symbol: { base: string; quote: string },
): FeedEvent {
  if (message.stream.endsWith('@trade')) {
    const trade = message.data as BinanceTrade
    return {
      type: 'trade',
      symbol,
      price: trade.p,
      quantity: trade.q,
      aggressor: trade.m ? 'Sell' : 'Buy',
      timestamp: trade.T,
    }
  }
  const depth = message.data as BinanceDepth
  return {
    type: 'book_snapshot',
    symbol,
    last_update_id: depth.lastUpdateId,
    bids: depth.bids.map(level),
    asks: depth.asks.map(level),
  }
}

// Open a Binance trade + partial-book stream for `symbol` (in BASE/QUOTE form)
// and push parsed events to `feed`. Returns a function that closes the socket.
export function openBinanceFeed(symbol: string, feed: (event: FeedEvent) => void): () => void {
  const [base, quote] = symbol.split('/')
  if (!base || !quote) {
    throw new Error(`bad symbol (expected BASE/QUOTE): ${symbol}`)
  }
  const sym = { base, quote }
  const stream = (base + quote).toLowerCase()
  const url = `wss://stream.binance.com:9443/stream?streams=${stream}@trade/${stream}@depth20@100ms`
  const ws = new WebSocket(url)

  ws.onmessage = (msg: MessageEvent<string>) => {
    feed(binanceEvent(JSON.parse(msg.data) as StreamMessage, sym))
  }

  return () => {
    ws.close()
  }
}
