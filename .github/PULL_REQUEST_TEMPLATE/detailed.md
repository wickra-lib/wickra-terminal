<!--
The long form. The short template is the default; this one is for a change that
touches the core contract, several bindings at once, or the release pipeline.
Delete any section that does not apply.
-->

## Summary

<!-- One to three sentences: what does this change, and why? -->

## Type of change

- [ ] Bug fix (non-breaking)
- [ ] New feature (non-breaking)
- [ ] Breaking change (alters the config, the command surface or a frame)
- [ ] Performance
- [ ] Refactor (no functional change)
- [ ] Documentation only
- [ ] CI / build / tooling

## Affected surfaces

- [ ] Core (`crates/wickra-terminal-core`)
- [ ] TUI renderer (`crates/ui-tui`)
- [ ] Web renderer (`web/`)
- [ ] Python binding (`bindings/python`)
- [ ] Node.js binding (`bindings/node`)
- [ ] WASM binding (`bindings/wasm`)
- [ ] C ABI (`bindings/c`) — and with it C++, C#, Go, Java, R
- [ ] C# (`bindings/csharp`)
- [ ] Go (`bindings/go`)
- [ ] Java (`bindings/java`)
- [ ] R (`bindings/r`)
- [ ] Examples / docs

## Linked issues

<!-- "Closes #123", "Refs #456". One per line. -->

Closes #

## The boundary

<!--
Answer only if this touched the core. Two rules hold this repository together
and both are easy to break without noticing:
-->

- [ ] Panels emit **view-models**, never renderer commands
- [ ] The `AppState` fold stays **O(1)** per event — nothing recomputes over history
- [ ] A new panel or field appears in **both** renderers, not only the one I ran
- [ ] A new command is reachable from a renderer, not only over FFI

## Schema and the golden corpus

- [ ] No frame or config schema change
- [ ] Schema changed, golden fixtures regenerated, and the change is described in `CHANGELOG.md`
- [ ] Cross-language golden tests pass (`c`, `csharp`, `go`, `java`, `node`, `python`, `r`)

## How was this tested?

<!--
Unit tests under `crates/*/tests/`, the registry completeness suite, property
tests, the fuzz targets, the per-binding suites, and anything you ran by hand.
A synthetic source is deterministic on every platform, so a repro on `synth:<seed>`
is one a reviewer can run.
-->

## Performance impact (if applicable)

| Benchmark | Before | After | Δ |
| --- | --- | --- | --- |
|  |  |  |  |

## Checklist

- [ ] `cargo fmt --all` is clean
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` is clean
- [ ] `cargo test --workspace --all-features` passes
- [ ] Binding suites run for every binding I touched
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] Counts that are written down still match the code (indicator count, panel count, benchmark numbers)
- [ ] No `todo*.md` or other local-only notes are staged
- [ ] Commits are signed, and carry no co-authored or generated-by trailers

## Notes for reviewers

<!-- What to look at first, known follow-ups, deliberately out-of-scope items. -->
