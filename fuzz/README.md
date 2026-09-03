# Fuzzing

Coverage-guided fuzzing for every path that takes bytes from outside the
process: a config file, a feed, and the JSON command boundary the ten bindings
drive.

`fuzz/` is a **detached crate** — `exclude`d from the workspace in the root
`Cargo.toml` — because `cargo-fuzz` builds it on nightly with sanitizer flags
that must not reach the rest of the build. That is also why `fuzz/Cargo.lock` is
not tracked: `cargo-fuzz init` ignores it, and the smoke job resolves fresh.

## The targets

| Target | What it feeds | What must hold |
| --- | --- | --- |
| `config_parse` | arbitrary bytes as a TOML config, as a JSON config, and into `Terminal::new` | no panic; malformed input is a clean `Err` |
| `feed_event` | arbitrary bytes as the public `Event`, and as a `Vec<Event>` feed | no panic; malformed input is a clean `Err` |
| `state_fold` | a parsed feed folded into a fresh `AppState` | no sequence of events panics, however adversarial — huge volumes, crossed books, negative sizes |
| `view_model` | arbitrary command JSON against a synth terminal | every command, known or malformed, returns a typed result without panicking, and the frame it produces serialises |
| `registry_drive` | a registered indicator, profile or bar type built by index with arbitrary parameters, then driven across all nine input families | `build` refuses bad parameters rather than panicking, and no registered kind panics on a structurally valid tick |

Between them they cover the four ways input a maintainer did not choose reaches
the core: the config a user writes, the feed a source produces, the command
string a binding passes through the C ABI, and the parameters a user names an
indicator with.

## Running them

Needs a nightly toolchain — `libfuzzer-sys` does not build on stable — and
`cargo-fuzz`:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

Then, from the repository root:

```bash
cd fuzz
cargo +nightly fuzz run state_fold                       # until you stop it
cargo +nightly fuzz run state_fold -- -max_total_time=60 # bounded
cargo +nightly fuzz list                                 # what is available
```

On Windows a target triple is required, which is what CI passes:

```bash
cargo +nightly fuzz run --target x86_64-unknown-linux-gnu state_fold
```

## What CI does

The `Fuzz (smoke)` job runs all four for 30 seconds each on every push and pull
request. Thirty seconds finds a crash that a change just introduced; it is not a
substitute for a long run, and is not meant to be. A longer soak before a
release is worth doing by hand.

The same job also gates `fuzz/` on `cargo +nightly fmt --check` and
`cargo +nightly clippy --all-targets -- -D warnings`, because the crate is
excluded from the workspace and would otherwise be the one corner of the
repository that neither reaches.

## When a target finds something

`cargo-fuzz` writes the input to `fuzz/artifacts/<target>/`. Reproduce it with:

```bash
cargo +nightly fuzz run <target> fuzz/artifacts/<target>/<file>
```

Fix the panic in the core rather than in the target: a target that stops
producing the input is a target that stopped testing. If the input is genuinely
invalid and the code should reject it, the fix is to return an `Err` — not to
filter the input before it reaches the code.
