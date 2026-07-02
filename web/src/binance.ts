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

interface BinanceTrade {
  p: string
  q: string
  m: boolean
  T: number
}

interface BinanceDepth {
  lastUpdateId: number
  bids: [string, string][]
  asks: [string, string][]
}

interface StreamMessage {
  stream: string
  data: BinanceTrade | BinanceDepth
}

function level(pair: [string, string]): { price: string; quantity: string } {
  return { price: pair[0], quantity: pair[1] }
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
    const parsed = JSON.parse(msg.data) as StreamMessage
    if (parsed.stream.endsWith('@trade')) {
      const trade = parsed.data as BinanceTrade
      feed({
        type: 'trade',
        symbol: sym,
        price: trade.p,
        quantity: trade.q,
        // Binance flags whether the buyer is the maker; if so the aggressor is
        // the seller, otherwise the buyer.
        aggressor: trade.m ? 'Sell' : 'Buy',
        timestamp: trade.T,
      })
    } else {
      const depth = parsed.data as BinanceDepth
      feed({
        type: 'book_snapshot',
        symbol: sym,
        last_update_id: depth.lastUpdateId,
        bids: depth.bids.map(level),
        asks: depth.asks.map(level),
      })
    }
  }

  return () => {
    ws.close()
  }
}
