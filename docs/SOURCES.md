# Sources

A source is a feed the terminal subscribes to and drains. Every source
implements the [`DataSource`](../crates/terminal-core/src/source/mod.rs) trait:

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
| `Live` | `Live { venue, symbol, testnet }` | the ten-venue [wickra-exchange](https://github.com/wickra-lib/wickra-exchange) connectivity layer (native builds only — the `live` feature). |
| `Replay` | `Replay { dataset }` | a recorded feed (a JSON array of events) with a movable cursor (the time-machine). Filesystem-free, so it runs in the browser too. |
| `Synth` | `Synth { seed }` | a deterministic synthetic feed — no network, reproducible, the default demo source. |

Sources are **activatable modules**: multiple run at once, they can be added,
removed and hot-swapped at runtime, and every symbol is keyed by
`(SourceId, Symbol)` for O(1) multi-symbol state.

## The `live` feature and WebAssembly

`Live` wraps the native exchange client, which needs raw sockets and cannot run
in a browser sandbox. It is gated behind the `live` feature (on by default for
native builds). The WASM binding builds `terminal-core` without `live`, so the
core compiles to `wasm32`; the browser feeds a live source over its own
WebSocket instead.

## The command surface

At runtime, sources and subscriptions are driven through the data-driven
[`command_json`](RENDERERS.md) boundary:

```json
{"type":"AddSource","spec":{"Synth":{"seed":2}}}
{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}
{"type":"Unsubscribe","source":0,"symbol":"BTC/USDT"}
{"type":"RemoveSource","id":1}
```

See also: [PANELS.md](PANELS.md) · [STREAMING.md](STREAMING.md).
