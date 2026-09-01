<p align="center">
  <a href="https://wickra.org"><img src="https://raw.githubusercontent.com/wickra-lib/.github/main/profile/wickra-banner.webp?v=514" alt="Wickra Terminal — the C++ API" width="100%"></a>
</p>

[![Built on Wickra](https://img.shields.io/badge/built%20on-wickra-3b82f6)](https://github.com/wickra-lib/wickra)
[![CI](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/ci.svg)](https://github.com/wickra-lib/wickra-terminal/actions/workflows/ci.yml)
[![codecov](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/codecov.svg)](https://codecov.io/gh/wickra-lib/wickra-terminal)
[![License: MIT OR Apache-2.0](https://raw.githubusercontent.com/wickra-lib/.github/main/profile/badges/wickra-terminal/license.svg)](https://github.com/wickra-lib/wickra-terminal#license)

# Wickra Terminal — C++

---

> **▶ Web renderer:** the same core drives a browser front-end (WASM + Vue) as a second renderer — see [`web/`](https://github.com/wickra-lib/wickra-terminal/tree/main/web).

The C++ API for [wickra-terminal](https://github.com/wickra-lib/wickra-terminal): a header-only
RAII wrapper over the [C ABI](https://github.com/wickra-lib/wickra-terminal/tree/main/bindings/c),
shipped as `wickra_terminal.hpp` beside the C header it wraps.

C++ can call the C ABI directly — `wickra_terminal.h` is already
`extern "C"`-guarded — and for a long time that was the whole C++ story. What it
cannot do directly is *own* anything: every handle and every returned string has
to be freed by hand, on every path including the ones an exception takes. This
header adds ownership and nothing else. It declares no indicator logic, holds no
state of its own, and compiles to the calls a careful author would have written.
It just makes the careless version impossible.

## Requirements

- A C++17 compiler
- The C ABI library to link against (`cargo build -p wickra-terminal-c --release`)

Both headers ship in the `wickra-terminal-c-<triple>.tar.gz` release artefact, so
a consumer downloading the ABI gets the ownership layer with it.

## Surface

```cpp
#include "wickra_terminal.hpp"

namespace wickra::terminal {

class Error : public std::runtime_error {
    int code() const noexcept;              // WICKRA_TERMINAL_ERR / _ERR_NULL
};

class Terminal {
    explicit Terminal(const std::string &config_json);   // throws Error
    std::string command(const std::string &cmd_json);    // throws Error
    bool valid() const noexcept;                         // false after a move

    Terminal(Terminal &&) noexcept;                      // move-only
    Terminal(const Terminal &) = delete;
};

std::string version();

}
```

What the header guarantees, and what each guarantee is there to prevent:

- **The handle has one owner.** Copying is deleted, because two owners of one
  handle would free it twice. Moving is allowed and leaves the source empty, so
  a moved-from destructor is a no-op.
- **A returned string is always freed.** `wickra_terminal_command` allocates on
  both exits — a frame on success, an error message on failure — and the failure
  exit throws. A guard frees it however the scope is left.
- **A failure is an exception.** `Error` carries the ABI's own message and its
  status code, rather than a return code a caller can ignore.
- **A moved-from `Terminal` throws** rather than passing a null handle across
  the boundary.
- **A panic does not unwind into C++.** The C ABI catches at its entry points;
  the release profile is `panic = "unwind"` precisely so it can.

## Example

```cpp
#include "wickra_terminal.hpp"
#include <cstdio>

using wickra::terminal::Terminal;

int main() {
    Terminal term(
        R"({"sources":[{"Synth":{"seed":1}}],)"
        R"("layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}})");

    (void)term.command(R"({"type":"Subscribe","source":0,"symbol":"BTC/USDT"})");

    std::string frame;
    for (int i = 0; i < 20; i++) {
        frame = term.command(R"({"type":"Tick"})");
    }
    std::printf("%s\n", frame.c_str());
}
```

Runnable examples live in [`examples/c/`](https://github.com/wickra-lib/wickra-terminal/tree/main/examples/c),
built and run in CI on all three platforms via CMake and ctest.

## Build

```bash
cargo build -p wickra-terminal-c --release
cmake -S examples/c -B examples/c/build
cmake --build examples/c/build --config Release
```

`CMAKE_CXX_STANDARD` is 17; the header uses a nested namespace and
`[[nodiscard]]`.

## Test

```bash
ctest --test-dir examples/c/build -C Release --output-on-failure
```

Seven tests, four of them C++: `terminal` drives the API, `golden_cpp` replays
the shared corpus through it, `streaming_test_cpp` checks that streaming a feed
and re-folding it in one batch reach byte-identical frames, and `lifetime`
checks the ownership rules — that copying is rejected at compile time, that a
moved-from terminal is empty and throws, that 500 failed commands leave the
terminal usable, and that a failure carries the ABI's message rather than a
placeholder.

They live under `examples/c/` rather than beside this README, and that is
deliberate rather than an accident of history. The C++ surface is a header
inside the C binding — `bindings/c/include/wickra_terminal.hpp` — so its tests
link the same library the C tests link, and one CMake project builds both. That
is also what the repository blueprint prescribes: *`examples/c/CMakeLists.txt`
builds the C and the C++ variants*. Compiling them the way a consumer compiles
them is the point; a second project for four files would trade that for a tidier
directory listing.

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
| `ListIndicators` | The catalogue: every indicator, profile and bar type this build accepts, each with its default parameters |
| `ReplayPosition` | Where a replayable source stands, for a time-machine scrubber. Answers `0/0` for a source that is not a recording |
| `ExportRecording` / `SetRecording` | Save the recorded events in the shape `Replay` takes, and turn recording on or off |

`ListIndicators` is the one command that answers rather than renders, and each
row carries `needs_reference`, which marks the pairwise indicators that compare
two markets and so require a `reference` symbol in their spec. A row for one of
the two friendly aliases also carries `alias_of` naming the canonical kind it
builds, so `Macd` and `MacdIndicator` read as one indicator rather than two.

A frame is `{"panels": [...]}`, one entry per configured panel, each tagged with
its `panel` kind — `chart`, `book`, `tape`, `watchlist`, `footprint`, `profile`, `bars`. See
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
