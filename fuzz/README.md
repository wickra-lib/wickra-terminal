# Fuzzing Wickra Terminal

Coverage-guided fuzzing for every path that takes bytes from outside the
process: a config file, a feed, and the JSON command boundary the ten bindings
drive.

## Setup

```bash
cargo install cargo-fuzz
rustup toolchain install nightly-2026-07-01
```

The date is the family's fuzz nightly, pinned in `ci.yml`: a rolling `nightly`
regressed with a codegen ICE unrelated to this code, so every repository moves
the date together, on purpose.

## Targets

| Target | What it exercises |
| --- | --- |
| `feed_event` | The event-parsing path: arbitrary bytes are deserialized as the public `Event` type and as a feed (a `Vec<Event>`). |
| `state_fold` | The state fold: arbitrary bytes are parsed as a feed and folded into a fresh `AppState`. |
| `view_model` | The data-driven command boundary: a synth terminal is driven with arbitrary command JSON. |
| `config_parse` | The config-parsing path: arbitrary bytes are parsed as a TOML config, a JSON config, and used to construct a terminal. |
| `registry_drive` | The generated registry: construct a registered kind with arbitrary parameters and drive it across all nine input families. |

## Run

```bash
# From the repository root:
cargo +nightly-2026-07-01 fuzz run --target x86_64-unknown-linux-gnu feed_event
cargo +nightly-2026-07-01 fuzz run --target x86_64-unknown-linux-gnu state_fold
cargo +nightly-2026-07-01 fuzz run --target x86_64-unknown-linux-gnu view_model
cargo +nightly-2026-07-01 fuzz run --target x86_64-unknown-linux-gnu config_parse
cargo +nightly-2026-07-01 fuzz run --target x86_64-unknown-linux-gnu registry_drive
```

Each run continues until a crash is found or it is interrupted. A short
time-boxed smoke run is what CI does:

```bash
cargo +nightly-2026-07-01 fuzz run --target x86_64-unknown-linux-gnu feed_event -- -max_total_time=30
```

The expectation for every target is that it never panics: malformed or
adversarial input must surface as an `Err` or an in-band error, never a crash.
