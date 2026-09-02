// A browser-side bridge from a Binance market-data WebSocket into the terminal's
// data-driven boundary. The WASM core cannot open native sockets, so the browser
// opens the public stream, parses each message into the core's `Event` JSON, and
// pushes it in through the `Feed` command on a `Manual` source. Public market
// data only — no API keys, no orders.
//
// Binance alone, and that is a limit rather than a design: the native source
// reaches ten venues through `wickra-exchange`, and each speaks a stream dialect
// that would have to be written again here in TypeScript. What this file owes a
// reader is to say so where they will read it — `docs/RENDERERS.md`,
// `web/README.md` and the app's own placeholder — rather than to look general
// and fail on the second venue anyone tries.

/** The first wait after the socket drops. The native source's number. */
const RECONNECT_MIN_WAIT_MS = 250

/** The longest wait between reconnect attempts. The native source's number. */
const RECONNECT_MAX_WAIT_MS = 30_000

/** How many historical bars a fresh subscription asks the REST API for. */
const BACKFILL_LIMIT = 200

export interface FeedEvent {
  type: 'trade' | 'book_snapshot' | 'ticker'
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

/** Binance's rolling ticker: last, best bid and ask, and base-asset volume. */
export interface BinanceTicker {
  c: string
  b: string
  a: string
  v: string
}

export interface StreamMessage {
  stream: string
  data: BinanceTrade | BinanceDepth | BinanceTicker
}

/** What the bridge is doing, so a status line can say which. */
export type FeedState = 'open' | 'reconnecting' | 'closed'

export interface FeedHandle {
  /** Stop reconnecting and close the socket. */
  close: () => void
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
  if (message.stream.endsWith('@ticker')) {
    // The quote and the turnover, which the bridge used to leave on the venue:
    // it subscribed trades and depth only, so a browser watchlist could never
    // show a spread or a volume however well the core folded them.
    const ticker = message.data as BinanceTicker
    return {
      type: 'ticker',
      symbol,
      last: ticker.c,
      bid: ticker.b,
      ask: ticker.a,
      volume: ticker.v,
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

/** Split `BASE/QUOTE` into its halves, refusing anything else. */
export function splitSymbol(symbol: string): { base: string; quote: string } {
  const [base, quote] = symbol.split('/')
  if (!base || !quote) {
    throw new Error(`bad symbol (expected BASE/QUOTE): ${symbol}`)
  }
  return { base, quote }
}

/**
 * Binance kline rows as trade events, oldest first.
 *
 * The native source seeds a fresh subscription from the venue's klines and the
 * browser bridge had no equivalent: it feeds a `Manual` source, which has no
 * `backfill`, so the chart opened empty on a market that has traded for years
 * and a bar indicator stayed silent for its whole warmup in wall-clock time.
 *
 * Each bar becomes one print at its close, which is what a bar carries and what
 * every charting platform warms an indicator on. The volume rides with it so the
 * bar the core rebuilds is the venue's rather than a synthetic one of size zero,
 * and the timestamp is the bar's close so the rebuilt bars land where the
 * venue's did instead of all inside the bar that is open now.
 */
export function klinesAsEvents(
  rows: unknown[],
  symbol: { base: string; quote: string },
): FeedEvent[] {
  return rows.flatMap((row) => {
    if (!Array.isArray(row) || row.length < 7) {
      return []
    }
    const close = String(row[4])
    const volume = String(row[5])
    const closeTime = Number(row[6])
    if (!Number.isFinite(closeTime)) {
      return []
    }
    return [
      {
        type: 'trade' as const,
        symbol,
        price: close,
        quantity: volume,
        aggressor: 'Buy',
        timestamp: closeTime,
      },
    ]
  })
}

/**
 * Fetch the venue's recent bars for `symbol` and hand them over as events.
 *
 * Best effort, exactly as the native source's backfill is: a venue that does not
 * carry the interval, a request that times out and a market too new to have a
 * history all mean a terminal that starts with no history, never one that
 * refuses to open the market.
 */
export async function backfill(
  symbol: string,
  interval = '1m',
  limit = BACKFILL_LIMIT,
): Promise<FeedEvent[]> {
  const sym = splitSymbol(symbol)
  const pair = (sym.base + sym.quote).toUpperCase()
  try {
    const response = await fetch(
      `https://api.binance.com/api/v3/klines?symbol=${pair}&interval=${interval}&limit=${limit}`,
    )
    if (!response.ok) {
      return []
    }
    return klinesAsEvents((await response.json()) as unknown[], sym)
  } catch {
    return []
  }
}

/**
 * Open a Binance trade, depth and ticker stream for `symbol` and push parsed
 * events to `feed`.
 *
 * Reconnects on its own. The socket used to carry `onmessage` and nothing else —
 * no `onerror`, no `onclose`, no retry — so a dropped connection left the chart
 * frozen at the last print with no indication that anything had happened, where
 * the native source has had an escalating backoff all along. This is the same
 * one: a quarter of a second doubling to half a minute, so a blip recovers
 * almost immediately and an endpoint that is simply gone is retried about twice
 * a minute rather than in a loop.
 *
 * `onState` reports open, reconnecting and closed, so a renderer can say which.
 */
export function openBinanceFeed(
  symbol: string,
  feed: (event: FeedEvent) => void,
  onState: (state: FeedState) => void = () => {},
): FeedHandle {
  const sym = splitSymbol(symbol)
  const stream = (sym.base + sym.quote).toLowerCase()
  const url =
    `wss://stream.binance.com:9443/stream?streams=` +
    `${stream}@trade/${stream}@depth20@100ms/${stream}@ticker`

  let socket: WebSocket | null = null
  let timer: ReturnType<typeof setTimeout> | undefined
  let wait = RECONNECT_MIN_WAIT_MS
  let stopped = false

  function schedule(): void {
    if (stopped) {
      return
    }
    onState('reconnecting')
    timer = setTimeout(connect, wait)
    wait = Math.min(wait * 2, RECONNECT_MAX_WAIT_MS)
  }

  function connect(): void {
    if (stopped) {
      return
    }
    const ws = new WebSocket(url)
    socket = ws

    ws.onopen = () => {
      // Reset once the socket is actually up, not on the attempt: resetting on
      // the attempt turns the backoff into a fixed quarter-second retry against
      // an endpoint that accepts connections and drops them.
      wait = RECONNECT_MIN_WAIT_MS
      onState('open')
    }

    ws.onmessage = (msg: MessageEvent<string>) => {
      // One malformed frame used to throw out of the handler. A feed is not a
      // contract; dropping the frame is the only response that keeps the rest
      // of the stream.
      let parsed: StreamMessage
      try {
        parsed = JSON.parse(msg.data) as StreamMessage
      } catch {
        return
      }
      if (typeof parsed?.stream !== 'string' || parsed.data === undefined) {
        return
      }
      feed(binanceEvent(parsed, sym))
    }

    // Both handlers, not one: a socket that errors also closes, so `onclose`
    // would carry it — but a browser that reports a blocked connection through
    // `onerror` alone would otherwise leave the bridge waiting forever.
    ws.onerror = () => {
      ws.close()
    }
    ws.onclose = () => {
      if (socket === ws) {
        socket = null
        schedule()
      }
    }
  }

  connect()

  return {
    close(): void {
      stopped = true
      if (timer !== undefined) {
        clearTimeout(timer)
      }
      onState('closed')
      const live = socket
      socket = null
      live?.close()
    },
  }
}
