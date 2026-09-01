---
name: Question / usage help
about: Ask how to do something with the terminal. For open-ended discussion prefer Discussions.
title: "[question] <short description>"
labels: ["question"]
assignees: []
---

> [!NOTE]
> If this is open-ended ("which panel layout should I use?") please use
> **Discussions** instead — issues are for actionable items.

## What are you trying to do?

<!-- The end goal on screen, not the API call. -->

## What have you tried?

<!--
The config, the commands, the docs you read, the search terms that did not
help. Show that you spent a few minutes before asking.
-->

```json
{ "sources": [{ "Synth": { "seed": 1 } }] }
```

## What is confusing or blocking you?

<!--
Specific question. "Why does Atr(14) stay null on a 1h timeframe?" beats
"indicators do not work" — and has an answer: only closed bars reach an
indicator, so a fourteen-period bar indicator needs fourteen bars.
-->

## Environment (only if relevant)

- `wickra-terminal` version: `e.g. 0.1.0`
- Renderer: `tui / web`
- Binding: `Rust / Python / Node.js / WASM / C / C++ / C# / Go / Java / R`
- Source: `e.g. synth:1, live:binance:BTC/USDT, replay`
