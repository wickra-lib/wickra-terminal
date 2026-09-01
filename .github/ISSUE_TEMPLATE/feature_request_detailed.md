---
name: Feature request (Detailed)
about: Long-form proposal with a config or command sketch, scope checkboxes, and contribution intent.
title: "[feat] <short description>"
labels: ["enhancement", "triage"]
assignees: []
---

## Problem / motivation

<!--
What are you trying to see or do that the terminal does not support today?
Describe it from the screen, not from the code.
-->

## Proposed shape

<!--
Sketch it as data. Almost everything here is config or a command, so a JSON
snippet says more than a paragraph — and if your idea cannot be written as one,
say so, because that is itself the interesting part.
-->

```json
{ "type": "AddIndicator", "spec": { "kind": "Atr", "params": [14] } }
```

## Scope

- [ ] New panel (a new view-model in the core, drawn by both renderers)
- [ ] New command on the `command_json` boundary
- [ ] New source kind
- [ ] Renderer-only: TUI
- [ ] Renderer-only: Web
- [ ] Reaching more of the `wickra-core` catalogue
- [ ] Performance
- [ ] Ergonomics / API cleanup
- [ ] Other (explain below)

## Does it belong in the core or in a renderer?

<!--
The rule this repository is built on: panels return view-models, never renderer
commands. Anything that has to look the same in the TUI and the browser belongs
in the core; anything that is a decision about drawing belongs in one renderer.
Say which you think it is, and why.
-->

## Reference / prior art

<!--
Link the terminal, platform or paper you would like this to match, and say what
it gets right. Screenshots are welcome.
-->

## Alternatives considered

<!-- What can you do today instead? Why is it not enough? -->

## Willingness to contribute

- [ ] I would like to implement this myself with guidance
- [ ] I can help review / test
- [ ] Requesting only — no bandwidth to implement
