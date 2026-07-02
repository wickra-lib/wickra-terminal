# Golden fixtures

Byte-exact fixtures pinning the deterministic feed-to-frame pipeline.

- `replay/<name>.json` — a recorded feed: a JSON array of market events.
- `expected/<name>.json` — the frame view-models the terminal produces after
  replaying that feed through the default layout.

`crates/terminal-core/tests/golden.rs` drives the feed through a `Terminal` and
asserts the produced frame equals `expected/<name>.json` byte-for-byte, so the
state fold and the panel view-models can never drift silently.

Because the same command protocol crosses every binding, these fixtures are also
the cross-language parity corpus: a binding can replay `replay/<name>.json` and
must reproduce `expected/<name>.json`.

## Regenerating

After an intentional change to the state fold or the view-model schema:

```bash
WICKRA_REGEN=1 cargo test -p terminal-core --test golden
```

Review the diff, then commit the updated fixtures.
