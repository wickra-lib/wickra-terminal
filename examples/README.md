# Wickra Terminal examples

Two scenarios, in every language.

## Rust — `examples/rust/`

| Example | What it does |
| --- | --- |
| `src/main.rs` | A runnable Rust example: drive a synthetic feed and print a frame. |

## C / C++ — `examples/c/`

Build the library first (`cargo build -p wickra-terminal-c --release`), then build and run
the examples via CMake, as the CI C ABI job does:

```bash
cmake -S examples/c -B examples/c/build
cmake --build examples/c/build --config Release
ctest --test-dir examples/c/build -C Release --output-on-failure
```

| Example | What it does |
| --- | --- |
| `lifetime.cpp` | Lifetime tests for the C++ RAII header. |
| `synth.c` | A minimal C example: drive a synthetic feed and print a frame. |
| `terminal.cpp` | A C++ example, over the shipped RAII header. |
| `time_machine.c` | A runnable C example: rewind a recorded feed and watch state re-fold. |

## C# — `examples/csharp/`

| Example | What it does |
| --- | --- |
| `Program.cs` | A runnable C# example: drive a synthetic feed and print a frame. |
| `TimeMachine.cs` | A runnable example against this binding. |

## Go — `examples/go/`

| Example | What it does |
| --- | --- |
| `synth_terminal.go` | A runnable Go example: drive a synthetic feed and print a frame. |
| `time_machine.go` | The time-machine scenario: rewind a recorded feed and watch state re-fold. |

## R — `examples/r/`

| Example | What it does |
| --- | --- |
| `synth_terminal.R` | A runnable R example: drive a synthetic feed and print a frame. |
| `time_machine.R` | A runnable example against this binding. |

## Java — `examples/java/`

| Example | What it does |
| --- | --- |
| `SynthTerminal.java` | A runnable Java example: drive a synthetic feed and print a frame. |
| `TimeMachine.java` | A runnable Java example: rewind a recorded feed and watch state re-fold. |

## Python — `examples/python/`

| Example | What it does |
| --- | --- |
| `synth_terminal.py` | A runnable Python example: drive a synthetic feed and print a frame. |
| `time_machine.py` | A runnable Python example: rewind a recorded feed and watch state re-fold. |

## Node.js — `examples/node/`

| Example | What it does |
| --- | --- |
| `synth_terminal.js` | A runnable Node.js example: drive a synthetic feed and print a frame. |
| `time_machine.js` | A runnable Node example: rewind a recorded feed and watch state re-fold. |

## WASM — `examples/wasm/`

Build the WASM package, serve the repository root, and open the page in a browser;
the module script inside it is what runs (CI parses it with `node --check`):

```bash
wasm-pack build bindings/wasm --target web
python -m http.server 8000     # then open http://localhost:8000/examples/wasm/
```

| Example | What it does |
| --- | --- |
| `index.html` | A runnable example against this binding. |

## Example datasets

The examples are self-contained: the spec and the input are inline, so there is
no shared `data/` directory to load. The cross-language golden fixtures, which
every binding is checked against byte for byte, live in [`../golden/`](../golden).

## Building the native library

The C, Go, C#, Java and R examples link the `wickra_terminal` C ABI:

```bash
cargo build --release -p wickra-terminal-c
```

## Python / Node.js

Build the binding first (`maturin develop` / `npm run build`), then run the
script. The Node example installs the local binding via a `file:` dependency.
