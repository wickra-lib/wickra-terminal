# Golden fixtures

Byte-exact fixtures pinning the deterministic feed-to-frame pipeline, and the
cross-language parity corpus for all ten language surfaces.

## The manifest

`manifest.json` is the index, and it is what makes the corpus extensible. Each
entry names a scenario:

```json
{
  "name": "basic",
  "config": "configs/basic.json",
  "commands": "commands/basic.txt",
  "expected": "expected/basic.min.json"
}
```

A binding reads the manifest, and for each scenario builds a terminal from the
config, replays the commands in order, and asserts the final frame equals the
expected file. Adding a scenario is one entry in `SCENARIOS` in
`crates/wickra-terminal-core/tests/golden.rs` plus a regeneration — no binding test
changes, in any language.

## The files

- `configs/<name>.json` — the complete `Terminal::new` config, so a binding
  builds the identical terminal from one file with no JSON assembly.
- `commands/<name>.txt` — the command sequence, one per line.
- `expected/<name>.min.json` — the frame exactly as `command_json` emits it
  (compact `serde_json::to_string`). Because every binding returns that string
  verbatim, byte equality against this one file is the exact parity check, with
  no per-language JSON deep-equal needed.
- `expected/<name>.json` — the same frame pretty-printed, for reading a diff.
- `replay/<name>.json` — the recorded feed, for scenarios that have one.
- `config.json` — a copy of the basic scenario's config, kept at the path the
  first corpus shipped.

The commands live in a file rather than an array inside the manifest so that
every manifest value stays a plain path. A command is a JSON string full of
quotes; embedding one would fill the manifest with escapes, and the two bindings
that deliberately carry no JSON dependency — Java and R — would have to unpick
them by hand. As it is, both read the manifest by splitting on the quote
character, which needs no parser and no regular expression.

## The scenarios

| Scenario | Pins |
|----------|------|
| `basic` | trades and a book snapshot through the default layout |
| `ticker` | the venue's ticker interleaved with trades, so the bid, ask, rolling volume and signed change on a watchlist row are pinned at real values rather than at zero |
| `panels` | the layout changed while it runs: a panel added with a depth, moved, and another taken off, so the frame a renderer draws follows the commands rather than the config it opened with |
| `book_deltas` | an L2 diff stream including level removals and new levels outside the previous range |
| `footprint` | repeated prices on both sides, so the per-price volume accumulates rather than recording one entry per price |
| `indicators` | a non-default indicator set (`Sma`, `Rsi`, `MacdIndicator`) driven far enough to produce real values, including the multi-output fields and the per-indicator series |
| `pairwise` | a pairwise indicator across two markets: the reference price reaches it through the tick, and the label carries which market it is against |
| `seek` | the time machine: drive to the end, rewind, drive forward again |
| `timeframe` | `SetTimeframe` mid-run, with a bar indicator that warms up again under the new size |
| `source_lifecycle` | a source added at run time replacing the one the terminal started with |
| `multi_source` | two sources at once, with focus on the second |

## Coverage

Nine language surfaces run every scenario: Rust, Python, Node, WASM, Go, C#,
Java, R, and C through `examples/c/golden.c`.

The C one used to run the basic scenario alone, which left the ABI hub — the one
every other binding routes through — as the least covered surface rather than
the most. It now walks the manifest like the others, and like the Java and R
suites it does so by splitting on the quote character rather than linking a JSON
parser, which is the whole reason every manifest value is a plain path.

## Regenerating

After an intentional change to the state fold or the view-model schema:

```bash
WICKRA_REGEN=1 cargo test -p wickra-terminal-core --test golden
```

Review the diff, then commit the updated fixtures. A frame that changed without
a deliberate schema change is the corpus doing its job.
