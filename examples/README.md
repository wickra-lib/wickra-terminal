# Examples

Two scenarios, in every language.

**`synth`** opens a deterministic `Synth` source, subscribes `BTC/USDT`, ticks
the terminal a few times and prints a frame of view-models — the shortest
program that drives the core.

**`time_machine`** plays a recorded feed to its end, rewinds to the second
trade, and shows the frame the forward pass had at that point. `Seek` throws the
folded state away and rebuilds it from the recording, so a rewind is
deterministic rather than approximate — which is what makes a recording more
than a slow synthetic feed, and it is the one capability a reader cannot guess
from the first scenario.

Neither is language-specific. Both are the same JSON commands over the same
three calls; what differs between the files is only how each language spells
them.

| Language | `synth` | `time_machine` |
|----------|---------|----------------|
| Rust | `cargo run -p wickra-terminal-example --bin synth` | `cargo run -p wickra-terminal-example --bin time_machine` |
| C | [`c/synth.c`](c/synth.c) | [`c/time_machine.c`](c/time_machine.c) |
| C++ | [`c/terminal.cpp`](c/terminal.cpp) | (the C example, through the same ABI) |
| Python | `python examples/python/synth_terminal.py` | `python examples/python/time_machine.py` |
| Node.js | `node examples/node/synth_terminal.js` | `node examples/node/time_machine.js` |
| WASM | [`wasm/`](wasm/) | (the same page — press *Rewind*) |
| Go | `cd examples/go && go run .` | `cd examples/go && go run . time-machine` |
| C# | `dotnet run --project examples/csharp` | `dotnet run --project examples/csharp -- time-machine` |
| Java | [`java/SynthTerminal.java`](java/SynthTerminal.java) | [`java/TimeMachine.java`](java/TimeMachine.java) |
| R | `Rscript examples/r/synth_terminal.R` | `Rscript examples/r/time_machine.R` |

Go and C# select the scenario with an argument rather than putting it in a
directory of its own. Go allows one `main` per package, and the C# project file
carries forty lines of native-library plumbing that would go stale in a second
copy — one program with two scenarios is the honest shape for both.

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

## Go

cgo links the library from `bindings/go/lib/<goos>_<goarch>/`, which CI stages
from the release build. Stage it once and the example runs from anywhere:

```bash
mkdir -p bindings/go/lib/linux_amd64        # or darwin_arm64, windows_amd64, ...
cp target/release/libwickra_terminal.so bindings/go/lib/linux_amd64/
cd examples/go && go run .
```

## C#

The project copies the native library beside the executable, so there is
nothing to stage:

```bash
dotnet run --project examples/csharp
```

## Java

FFM needs `--enable-native-access`, and the library directory is a system
property rather than a path lookup:

```bash
mvn -f bindings/java/pom.xml -q package -DskipTests
javac -cp bindings/java/target/classes examples/java/SynthTerminal.java -d examples/java/out
java --enable-native-access=ALL-UNNAMED -Dnative.lib.dir=target/release \
     -cp "bindings/java/target/classes:examples/java/out" SynthTerminal
```

The classpath separator is `:` on Linux and macOS and `;` on Windows.

## R

`WKTERM_INC` and `WKTERM_LIB` point the package at a local build instead of a
downloaded release asset:

```bash
export WKTERM_INC="$PWD/bindings/c/include" WKTERM_LIB="$PWD/target/release"
R CMD INSTALL bindings/r
Rscript examples/r/synth_terminal.R
```

On Windows the library directory also has to be on `PATH` for the DLL to load.

## Python / Node.js

Build the binding first (`maturin develop` / `npm run build`), then run the
script. The Node example installs the local binding via a `file:` dependency.
