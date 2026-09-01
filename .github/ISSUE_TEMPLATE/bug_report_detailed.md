---
name: Bug report (Detailed)
about: Long-form bug report with the config, the command sequence, and expected-vs-actual frames.
title: "[bug] <short description>"
labels: ["bug", "triage"]
assignees: []
---

## Summary

<!-- One or two sentences. What did you expect, what happened instead? -->

## Where it goes wrong

- [ ] Core state fold (wrong value in the frame, whatever the renderer)
- [ ] TUI renderer (`wickra-terminal` binary)
- [ ] Web renderer (`web/`)
- [ ] A binding's boundary (the frame JSON differs from Rust's)
- [ ] Docs / examples only

## Affected binding

- [ ] Rust (`wickra-terminal-core`)
- [ ] Python (`wickra-terminal`)
- [ ] Node.js (`wickra-terminal`)
- [ ] WASM (`wickra-terminal-wasm`)
- [ ] C ABI (`bindings/c`)
- [ ] C++ (`wickra_terminal.hpp`)
- [ ] C# (`WickraTerminal` on NuGet)
- [ ] Go (`github.com/wickra-lib/wickra-terminal-go`)
- [ ] Java (`org.wickra:wickra-terminal` on Maven Central)
- [ ] R (`bindings/r`)

## Environment

| Field | Value |
| --- | --- |
| `wickra-terminal` version | `e.g. 0.1.0` |
| Binding version | `e.g. python 0.1.0` |
| OS / arch | `e.g. Windows 11 x86_64, Linux glibc` |
| Terminal emulator (TUI only) | `e.g. Windows Terminal, kitty, tmux` |
| Browser (web only) | `e.g. Firefox 141` |
| Rust toolchain | `rustc --version`, if building from source |

## The terminal that reproduces it

<!--
The config the terminal was built from, and the commands applied to it in
order. Never paste API keys, secrets or signed request payloads — the terminal
needs none, so anything of the sort in a repro is a mistake worth redacting.
-->

```json
{ "sources": [{ "Synth": { "seed": 1 } }], "timeframe": "1m",
  "indicators": [{ "kind": "Sma", "params": [20] }] }
```

```json
{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}
{"type":"Tick"}
```

## Actual frame

```json
<paste the frame JSON, the panic, or the wrong values>
```

## Expected frame

<!--
What should the panel have shown? If this is an indicator value, name what you
checked it against — wickra itself, TA-Lib, a paper, another terminal.
-->

## Does it reproduce on a synthetic source?

<!--
`synth:<seed>` is deterministic on every platform, so a repro on it is one
anybody can run. If the bug only appears on a live feed, say which venue and
market, and roughly when.
-->

## Additional context

<!-- Redacted logs, screenshots of the panel, links to related issues. -->
