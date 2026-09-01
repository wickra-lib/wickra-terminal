# Documentation

The API reference for every binding lives at
**[terminal.wickra.org](https://terminal.wickra.org)** — one page per language,
each holding the same three-call surface (`new`, `command`, `version`) in that
language's idiom:

- [Rust](https://terminal.wickra.org/api/rust),
  [Python](https://terminal.wickra.org/api/python),
  [Node](https://terminal.wickra.org/api/node),
  [WASM](https://terminal.wickra.org/api/wasm),
  [C](https://terminal.wickra.org/api/c),
  [C#](https://terminal.wickra.org/api/csharp),
  [Go](https://terminal.wickra.org/api/go),
  [Java](https://terminal.wickra.org/api/java) and
  [R](https://terminal.wickra.org/api/r).

The site is a separate git repository at
`https://github.com/wickra-lib/wickra-terminal-site`. Open a pull request there
to change the reference; it is built with VitePress and deploys to
`terminal.wickra.org`.

## What is kept here instead

Six guides live beside the code rather than on the site, and that is a
deliberate difference from [wickra](https://github.com/wickra-lib/wickra), whose
`docs/` is only this signpost. Each of these describes a contract that changes
in the same commit as the code it documents — a reference that can drift a
release behind would be worse than none:

| Guide | What it settles |
|---|---|
| [INDICATORS.md](INDICATORS.md) | The registry: naming an indicator, the nine input families, what a tick feeds, pairwise references, profiles and alternative bars. |
| [PANELS.md](PANELS.md) | Every panel and the view-model it emits. |
| [RENDERERS.md](RENDERERS.md) | The `command_json` boundary: every command, its JSON shape, and the frame that comes back. |
| [SOURCES.md](SOURCES.md) | The `DataSource` trait and the four source kinds. |
| [STREAMING.md](STREAMING.md) | How events fold into state in O(1), and what that costs. |
| [Cookbook.md](Cookbook.md) | Worked configurations, end to end. |

Anything that is *not* one of those — quickstarts, prose, the pitch — belongs on
the site. Do not start a second documentation tree here.
