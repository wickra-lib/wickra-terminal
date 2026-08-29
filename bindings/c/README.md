<p align="center">
  <a href="https://wickra.org"><img src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/wickra-banner.webp?v=514" alt="Wickra Terminal — the C ABI hub" width="100%"></a>
</p>

[![Built on Wickra](https://img.shields.io/badge/built%20on-wickra-3b82f6)](https://github.com/wickra-lib/wickra)
[![CI](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/ci.svg)](https://github.com/wickra-lib/wickra-terminal/actions/workflows/ci.yml)
[![codecov](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/codecov.svg)](https://codecov.io/gh/wickra-lib/wickra-terminal)
[![License: MIT OR Apache-2.0](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/license.svg)](https://github.com/wickra-lib/wickra-terminal#license)

# Wickra Terminal — C ABI

---

> **▶ Web renderer:** the same core drives a browser front-end (WASM + Vue) as a second renderer — see [`web/`](https://github.com/wickra-lib/wickra-terminal/tree/main/web).

The C ABI hub for [wickra-terminal](https://github.com/wickra-lib/wickra-terminal): a `cdylib` + `staticlib` that every
C-capable language (C, C++, C#, Go, Java, R) links against. The surface is a
tiny, JSON-shaped data API — a handle in, command JSON in, frame JSON out.

Five exports carry all ten languages. `scripts/check_binding_surface.py` asserts
in CI that each of them reaches every binding, so a language cannot quietly fall
behind the header.

## Requirements

- A C99 compiler (the header is `cpp_compat`, so C++ links against it unchanged)
- `cbindgen` to regenerate the header

## Surface

```c
#include "wickra_terminal.h"

WickraTerminal *wickra_terminal_new(const char *config_json);
void            wickra_terminal_free(WickraTerminal *handle);
int             wickra_terminal_command(WickraTerminal *handle,
                                        const char *cmd_json,
                                        char **out_json);
void            wickra_terminal_free_string(char *s);
const char     *wickra_terminal_version(void); /* static — do not free */
```

- `wickra_terminal_new` builds a terminal from a JSON config; returns `NULL` on a
  null or invalid argument.
- `wickra_terminal_command` applies a command JSON and writes the resulting frame
  JSON to `*out_json`. Returns `0` (`WICKRA_TERMINAL_OK`) on success, `-2`
  (`WICKRA_TERMINAL_ERR`) with the error message in `*out_json`, or `-1`
  (`WICKRA_TERMINAL_ERR_NULL`) if a required pointer is null.
- The caller owns `*out_json` and frees it with `wickra_terminal_free_string`.
- Panics never cross the boundary: every entry point that runs code wraps it
  in `catch_unwind` and returns an error instead. The release profile is
  `panic = "unwind"` precisely so that it can -- with `abort` there would be
  nothing to catch, and a panic in the core would take the host process with
  it. A test reads this crate's own source and fails if an entry point is
  added without one.

## Example

```c
WickraTerminal *t = wickra_terminal_new(
    "{\"sources\":[{\"Synth\":{\"seed\":1}}],"
    "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}");

char *out = NULL;
wickra_terminal_command(t, "{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}", &out);
wickra_terminal_free_string(out);

wickra_terminal_command(t, "{\"type\":\"Tick\"}", &out); /* out = frame JSON */
printf("%s\n", out);
wickra_terminal_free_string(out);

wickra_terminal_free(t);
```

A runnable C and C++ example lives in [`examples/c/`](https://github.com/wickra-lib/wickra-terminal/tree/main/examples/c),
built and run in CI on all three platforms via CMake and ctest.

## Build

```bash
cargo build -p wickra-terminal-c --release
```

The library is named `wickra_terminal` (`.dll` / `.so` / `.dylib`, plus a
`.a`/`.lib` static library) under `target/release/`.

The header is generated and committed. Regenerate it with:

```bash
cbindgen --config bindings/c/cbindgen.toml --crate wickra-terminal-c \
  --output bindings/c/include/wickra_terminal.h
```

`.github/scripts/check-cbindgen.sh` verifies the committed header still matches
the Rust surface; CI runs the same script, and it skips cleanly if `cbindgen` is
not installed.

## Test

```bash
cargo test -p wickra-terminal-c
```

## The command protocol

Every binding drives the same thirteen commands, and the frame that comes back is
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
| `ListIndicators` | The catalogue: every registry name with its default parameters |

`ListIndicators` is the one command that answers rather than renders, and each
row carries `needs_reference`, which marks the pairwise indicators that compare
two markets and so require a `reference` symbol in their spec. A row for one of
the two friendly aliases also carries `alias_of` naming the canonical kind it
builds, so `Macd` and `MacdIndicator` read as one indicator rather than two.

A frame is `{"panels": [...]}`, one entry per configured panel, each tagged with
its `panel` kind — `chart`, `book`, `tape`, `watchlist`, `footprint`. See
[`docs/`](https://github.com/wickra-lib/wickra-terminal/tree/main/docs) for the panel and source references.

## Cross-language equality

The same config and the same command sequence produce a byte-identical frame in
Rust, Python, Node.js, WASM, C, C++, C#, Go, Java and R. That is not an aspiration:
[`golden/`](https://github.com/wickra-lib/wickra-terminal/tree/main/golden) holds a recorded feed and the expected frame,
and every binding's test suite asserts its own output against that one file.

## Documentation

- **Repository:** <https://github.com/wickra-lib/wickra-terminal>
- **Panels, sources, renderers, streaming:** [`docs/`](https://github.com/wickra-lib/wickra-terminal/tree/main/docs)
- **Cookbook:** [`docs/Cookbook.md`](https://github.com/wickra-lib/wickra-terminal/blob/main/docs/Cookbook.md)
- **Built on Wickra:** <https://github.com/wickra-lib/wickra> · <https://docs.wickra.org>

## Security

Found a security issue? **Please don't open a public issue.** Report it privately
via the repository's *Security* tab (*"Report a vulnerability"*) or email
**support@wickra.org**. Full policy: <https://github.com/wickra-lib/wickra-terminal/blob/main/SECURITY.md>.

## Disclaimer

Not a trading system, and not financial advice. The terminal renders market data
and derived view-models; what you do with them is your own risk. Provided **as
is**, without warranty of any kind.

## License

Dual-licensed under [MIT](https://github.com/wickra-lib/wickra-terminal/blob/main/LICENSE-MIT) or
[Apache-2.0](https://github.com/wickra-lib/wickra-terminal/blob/main/LICENSE-APACHE), at your option.
