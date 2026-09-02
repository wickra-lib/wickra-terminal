# Sources

A source is a feed the terminal subscribes to and drains. Every source
implements the [`DataSource`](../crates/wickra-terminal-core/src/source/mod.rs) trait:

```rust
trait DataSource {
    fn id(&self) -> SourceId;
    fn kind(&self) -> SourceKind;
    fn subscribe(&mut self, sym: &Symbol) -> Result<()>;
    fn unsubscribe(&mut self, sym: &Symbol);
    fn poll(&mut self) -> Vec<(Symbol, Event)>;   // symbol-tagged market events
}
```

`poll()` yields only market events (trades, ticker, book snapshot/diff);
connection-lifecycle events without a symbol are handled at the source boundary
and never reach the state fold.

## Source kinds

| Kind | Spec | Feeds from |
|------|------|-----------|
| `Live` | `Live { venue, symbol, testnet, market }` | the ten-venue [wickra-exchange](https://github.com/wickra-lib/wickra-exchange) connectivity layer (native builds only — the `live` feature). |
| `Replay` | `Replay { dataset }` | a recorded feed (a JSON array of events) with a movable cursor (the time-machine). Filesystem-free, so it runs in the browser too. |
| `Synth` | `Synth { seed }` | a deterministic synthetic feed — no network, reproducible, the default demo source. |
| `Manual` | `"Manual"` | a host-fed source: the core opens no connection; the host pushes events in through the `Feed` command. The browser bridges an exchange WebSocket into it. |

Sources are **activatable modules**: multiple run at once, they can be added,
removed and hot-swapped at runtime, and every symbol is keyed by
`(SourceId, Symbol)` for O(1) multi-symbol state.

## The `live` feature and WebAssembly

`Live` wraps the native exchange client, which needs raw sockets and cannot run
in a browser sandbox. It is gated behind the `live` feature (on by default for
native builds). The WASM binding builds `wickra-terminal-core` without `live`, so the
core compiles to `wasm32`. In the browser, live data instead comes through a
`Manual` source: the page opens the exchange WebSocket itself and pushes each
message in with the `Feed` command. The web renderer ships a Binance bridge
(`web/src/binance.ts`) that does exactly this — public market data only, no keys.

## The command surface

At runtime, sources and subscriptions are driven through the data-driven
[`command_json`](RENDERERS.md) boundary:

Ids are assigned in order as sources are added, starting after the ones the
config opened. These examples assume the config opened one, so it holds id 0.

```json
{"type":"AddSource","spec":{"Synth":{"seed":2}}}
{"type":"AddSource","spec":"Manual"}
{"type":"Subscribe","source":2,"symbol":"BTC/USDT"}
{"type":"Feed","source":2,"event":{"type":"trade","symbol":{"base":"BTC","quote":"USDT"},"price":"64000","quantity":"0.1","aggressor":"Buy","timestamp":1}}
{"type":"Unsubscribe","source":2,"symbol":"BTC/USDT"}
{"type":"RemoveSource","id":1}
```

## Which market, and how much history

`market` picks the venue's book: `Spot` (the default), `UsdMFutures`,
`CoinMFutures` or `Margin`. It was hard-coded to spot, so a perpetual could not
be opened at all — which left the whole derivatives side of the catalogue with
no market to watch, before the question of a funding feed even arises. The
source shorthand takes it as a third segment:

```text
live:binance:BTC/USDT          spot
live:binance:BTC/USDT:usdm     USD-margined perpetual
live:binance:BTC/USDT:coinm    coin-margined
```

A symbol carries a slash and never a colon, so a second colon can only be the
market's — and a word there that is not a market name is reported rather than
folded into the symbol, which is what used to happen and produced a message
about an unknown symbol for what was a typo in the market.

`backfill` is how many historical bars a fresh subscription fetches, 200 by
default and 0 to turn it off. Without it every bar came from ticks the terminal
saw itself: `Atr(14)` on an hourly timeframe was silent for fourteen hours, and
the chart opened empty on a market that has traded since 2017. The bars seed the
chart, the price history and every bar-derived indicator; the book, the tape and
the footprint are not seeded, because a bar records that trading happened rather
than the prints it was made of, and inventing those would put numbers on screen
that no venue published.

A failed backfill is not a failed subscription. The venue may not carry the
interval, the request may time out, or the market may be too new to have a
history — and the right outcome in each is a terminal that starts with no
history, not one that refuses to open the market.

> **Funding and open interest still have no live feed, and that is upstream.**
> `wickra-exchange-core` defines `DerivativesFeed` and `DerivativesTickBuilder`,
> but no venue implements them and the `Exchange` trait exposes no way to
> subscribe to one. So the `DerivativesTick` indicator family can only be driven
> through the `FeedDerivatives` command, from a host with its own source — which
> is why that command exists. Opening a perpetual is now possible; streaming its
> funding is not, and will not be until the exchange layer grows the channel.

## Making a recording

`Replay` takes a feed and there was no way to produce one: nothing in the
repository wrote a session out, and the `dataset` field takes the events
themselves rather than a path, so the only way to get a recording was to already
have one. The terminal could rewind and could not record.

Set a capacity and it keeps that many recent events:

```json
{ "sources": [{ "Live": { "venue": "binance", "symbol": "BTC/USDT" } }], "record": 50000 }
```

Off by default, and deliberately a ring rather than a log: a terminal left
running overnight must not grow without limit, and what a person reaches for a
recorder for is the last few minutes rather than the whole session.

```json
{"type":"ExportRecording"}
{"type":"SetRecording","capacity":50000}
{"type":"SetRecording","capacity":null}
```

`ExportRecording` answers with the events in exactly the shape `Replay` takes,
so the output goes straight back in as a `dataset`. The core stays
filesystem-free — it has to, to run in a browser — so it records into memory and
hands the events over; writing them anywhere is the host's job. The TUI binds
`w` to writing them beside itself, named by the wall clock rather than
overwriting one path, because what a person saves is a moment they want to keep.

`SetRecording` clears what is already held, on and off alike: a capacity change
that kept the old events would leave a recording that is part one size and part
another.

Events are recorded as they are polled, not as they are folded. `fold` is also
how a seek re-folds a recording, so recording there would append the replayed
events back onto the recording and every rewind would double it.

`Feed` pushes an external market event into a `Manual` source; it is folded on
the next `Tick`, exactly like a pulled event. The event must be for a market
subscribed on that source.

A manual source holds at most 4,096 events between ticks. Past that, `Feed` is
an error naming the backlog rather than a queue that keeps growing -- the shape
a browser tab has when it is backgrounded and stops firing rAF while its socket
keeps delivering. The events already queued are refused rather than evicted, so
what is there stays contiguous: a book delta only means anything in sequence,
and dropping one leaves a local book that is wrong rather than merely stale.
Tick to drain, and the source takes events again.

## The time-machine

A `Replay` source records the whole feed, so it can be rewound. The `Seek`
command moves a replay source to a recorded position and re-folds the state for
its markets, then playback resumes forward from there:

```json
{"type":"Seek","source":0,"index":120}
```

State is rebuilt by **re-folding the feed** rather than restoring a snapshot: a
market's streaming indicators are not cloneable, so a snapshot ring is not
viable, and re-folding is deterministic and O(1) per event. `Seek` on a live or
synthetic source (which keep no recorded history) is an error. Because it is just
another command on the [data-driven boundary](RENDERERS.md), every binding and
both renderers get the time-machine for free.

See also: [INDICATORS.md](INDICATORS.md) · [PANELS.md](PANELS.md) · [STREAMING.md](STREAMING.md).
