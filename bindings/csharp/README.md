<p align="center">
  <a href="https://wickra.org"><img src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/wickra-banner.webp?v=514" alt="Wickra Terminal — the data-driven streaming trading terminal: one core in ten languages, a native TUI and a Web renderer" width="100%"></a>
</p>

[![CI](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/ci.svg)](https://github.com/wickra-lib/wickra-terminal/actions/workflows/ci.yml)
[![codecov](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/codecov.svg)](https://codecov.io/gh/wickra-lib/wickra-terminal)
[![NuGet](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/nuget.svg)](https://www.nuget.org/packages/WickraTerminal)
[![License: MIT OR Apache-2.0](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/license.svg)](https://github.com/wickra-lib/wickra-terminal#license)

# Wickra Terminal — C#

---

> **▶ Live demo:** the Wickra library's own 514 indicators over real Binance market data, computed live in your browser — **[live.wickra.org](https://live.wickra.org)** · zero backend, powered by `wickra-wasm`.

**One core. Ten languages. Two renderers — for C#. `dotnet add package WickraTerminal` — prebuilt native library, no system dependencies.**

.NET bindings for the [wickra-terminal](https://github.com/wickra-lib/wickra-terminal) data-driven core, over the Wickra
C ABI hub via P/Invoke. Build a `Terminal` from a JSON config, drive it with
command JSON, read back frame view-models — the same protocol as the native TUI
and every other binding.

## Install

```bash
dotnet add package WickraTerminal
```

The native library ships prebuilt per platform under `runtimes/<rid>/native/`,
selected automatically. There is nothing to compile. Targets .NET 8 and later.

### Requirements

- .NET 8.0 or newer
- The native `wickra_terminal` library on the loader path (the package ships it;
  a source build stages it, see below)

### Building from this repository (contributors)

```bash
cargo build -p wickra-terminal-c            # produces the native wickra_terminal library
dotnet test bindings/csharp/WickraTerminal.Tests
dotnet run --project examples/csharp        # the runnable example
```

Both the test project and the example copy the native library next to their
output so the default P/Invoke resolver finds it, on Windows, Linux and macOS.

## Quick start

```csharp
using WickraTerminal;

using var term = new Terminal(
    "{\"sources\":[{\"Synth\":{\"seed\":1}}]," +
    "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}");

term.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
string frame = term.Command("{\"type\":\"Tick\"}");
Console.WriteLine(frame);
Console.WriteLine(Terminal.Version());
```

### API

| Name | Returns |
|------|---------|
| `new Terminal(configJson)` | A handle; throws on an invalid config |
| `term.Command(cmdJson)` | The frame JSON; throws on an invalid command |
| `Terminal.Version()` | The library version |
| `term.Dispose()` | Releases the native handle |

`Terminal` implements `IDisposable` and carries a finaliser, so `using` is the
intended form but a dropped handle is still reclaimed.

### The command protocol

Every binding drives the same nineteen commands, and the frame that comes back is
the same JSON in all of them:

| Command | Effect |
|---------|--------|
| `Tick` | Poll every source, fold what arrived, return the frame |
| `Subscribe` / `Unsubscribe` | Add or drop a market on one source |
| `SetFocus` | Choose the market the panels render |
| `AddSource` / `RemoveSource` | Attach or detach a feed at run time |
| `Seek` | Rewind or fast-forward a replay source (the time machine) |
| `Feed` | Hand an event to a `Manual` source from the host |
| `FeedDerivatives` | Fold a derivatives update -- funding, open interest, positioning, mark/index/futures -- into a market |
| `AddIndicator` / `RemoveIndicator` | Track or drop an indicator on every market |
| `SetTimeframe` | Set the bar size the candle-input indicators are fed at |
| `ListIndicators` | The catalogue: every indicator, profile and bar type this build accepts, each with its default parameters |
| `ReplayPosition` | Where a replayable source stands, for a time-machine scrubber. Answers `0/0` for a source that is not a recording |
| `ExportRecording` / `SetRecording` | Save the recorded events in the shape `Replay` takes, and turn recording on or off |
| `AddPanel` / `RemovePanel` / `MovePanel` | Place a panel on the layout, take one off, or move and resize one -- the layout is no longer fixed at start-up |

`ListIndicators` is the one command that answers rather than renders, and each
row carries `needs_reference`, which marks the pairwise indicators that compare
two markets and so require a `reference` symbol in their spec. A row for one of
the two friendly aliases also carries `alias_of` naming the canonical kind it
builds, so `Macd` and `MacdIndicator` read as one indicator rather than two.

A frame is `{"panels": [...]}`, one entry per configured panel, each tagged with
its `panel` kind — `chart`, `book`, `tape`, `watchlist`, `footprint`, `profile`, `bars`. See
[`docs/`](https://github.com/wickra-lib/wickra-terminal/tree/main/docs) for the panel and source references.

### Cross-language equality

The same config and the same command sequence produce a byte-identical frame in
Rust, Python, Node.js, WASM, C, C++, C#, Go, Java and R. That is not an aspiration:
[`golden/`](https://github.com/wickra-lib/wickra-terminal/tree/main/golden) holds a recorded feed and the expected frame,
and every binding's test suite asserts its own output against that one file.

## Benchmark

`benchmarks/` reports this binding's throughput over the shared core. It measures
the call overhead of `[LibraryImport]` P/Invoke over the C ABI, not a cross-library ratio (the same Rust core runs
under every binding) — see the repository
[BENCHMARKS.md](https://github.com/wickra-lib/wickra-terminal/blob/main/BENCHMARKS.md) for the
numbers, the machine and how each harness is run.

## Documentation

The full guide, the spec reference and the API documentation live in the main
repository and the documentation site:

- **Repository:** <https://github.com/wickra-lib/wickra-terminal>
- **Docs** (guides, spec reference, cookbook): <https://terminal.wickra.org>
- **Runnable example:** [`examples/csharp/`](https://github.com/wickra-lib/wickra-terminal/tree/main/examples/csharp)

- **Repository:** <https://github.com/wickra-lib/wickra-terminal>
- **Panels, sources, renderers, streaming:** [`docs/`](https://github.com/wickra-lib/wickra-terminal/tree/main/docs)
- **Cookbook:** [`docs/Cookbook.md`](https://github.com/wickra-lib/wickra-terminal/blob/main/docs/Cookbook.md)
- **Built on Wickra:** <https://github.com/wickra-lib/wickra> · <https://docs.wickra.org>

Wickra Terminal ships native bindings for Python, Node.js, WASM and Rust, plus a C ABI hub that any
C-capable language (C, C++, C#, Go, Java, R) links against — all forwarding to the
same data-driven, `unsafe`-forbidden Rust core.

## Security

Found a security issue? **Please don't open a public issue.** Report it privately
via the repository's *Security* tab (*"Report a vulnerability"*) or email
**support@wickra.org** with a subject line starting `[wickra security]`. Full
policy: <https://github.com/wickra-lib/wickra-terminal/blob/main/SECURITY.md>.

## Disclaimer

This software is provided "as is", without warranty of any kind. It is a research
and engineering tool, **not financial advice**. Trading carries risk of loss. Run
in paper mode and against exchange testnets, and review the code before risking
real capital.

## License

Licensed under either of [Apache-2.0](https://github.com/wickra-lib/wickra-terminal/blob/main/LICENSE-APACHE)
or [MIT](https://github.com/wickra-lib/wickra-terminal/blob/main/LICENSE-MIT) at your option.
