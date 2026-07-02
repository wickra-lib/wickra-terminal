# Examples

One runnable example per language. Each opens a deterministic `Synth` source,
subscribes `BTC/USDT`, ticks the terminal a few times and prints a frame of
view-models — the same protocol the TUI and Web renderers drive.

| Language | Path | Run |
|----------|------|-----|
| Rust | [`rust/`](rust/) | `cargo run -p wickra-terminal-example` |
| C | [`c/synth.c`](c/synth.c) | see the C/C++ block below |
| C++ | [`c/terminal.cpp`](c/terminal.cpp) | see the C/C++ block below |
| Python | [`python/synth_terminal.py`](python/synth_terminal.py) | `python examples/python/synth_terminal.py` |
| Node.js | [`node/synth_terminal.js`](node/synth_terminal.js) | `node examples/node/synth_terminal.js` |
| Go | [`go/synth_terminal.go`](go/synth_terminal.go) | `cd examples/go && go run .` |
| C# | [`csharp/Program.cs`](csharp/Program.cs) | `dotnet run --project examples/csharp` |
| Java | [`java/SynthTerminal.java`](java/SynthTerminal.java) | see the header of the file |
| R | [`r/synth_terminal.R`](r/synth_terminal.R) | `Rscript examples/r/synth_terminal.R` |

## Building the native library

The C, Go, C#, Java and R examples link the `wickra_terminal` C ABI:

```bash
cargo build --release -p wickra-terminal-c
```

## C / C++

```bash
cmake -S examples/c -B examples/c/build
cmake --build examples/c/build --config Release
ctest --test-dir examples/c/build -C Release --output-on-failure
```

The CMake build copies the runtime DLL next to each executable on Windows and
caps each test's timeout, so a missing dependency fails fast instead of hanging.

## Python / Node.js

Build the binding first (`maturin develop` / `npm run build`), then run the
script. The Node example installs the local binding via a `file:` dependency.
