# Golden fixtures

Byte-exact fixtures pinning the deterministic feed-to-frame pipeline.

- `replay/<name>.json` — a recorded feed: a JSON array of market events.
- `expected/<name>.json` — the frame view-models the terminal produces after
  replaying that feed through the default layout (pretty, for human diffing).
- `expected/<name>.min.json` — the same frame exactly as `command_json` emits it
  (compact `serde_json::to_string`).
- `config.json` — the complete `Terminal::new` config: a `Replay` source with the
  feed embedded plus the default layout, so a binding builds the identical
  terminal from one file with no JSON assembly.

`crates/terminal-core/tests/golden.rs` drives the feed through a `Terminal` and
asserts the produced frame equals `expected/<name>.json` byte-for-byte, so the
state fold and the panel view-models can never drift silently.

Because the same command protocol crosses every binding, these fixtures are also
the cross-language parity corpus. Every binding ships a golden test that loads
`config.json`, subscribes `BTC/USDT` on source 0, ticks past the feed, and asserts
its frame equals `expected/basic.min.json` byte-for-byte. Since each binding
returns the core's compact `command_json` string verbatim, byte equality against
that one file is the exact cross-language parity check — no per-language JSON
deep-equal is required.

## Regenerating

After an intentional change to the state fold or the view-model schema:

```bash
WICKRA_REGEN=1 cargo test -p terminal-core --test golden
```

Review the diff, then commit the updated fixtures.
