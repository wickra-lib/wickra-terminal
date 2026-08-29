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
| Java | [`java/SynthTerminal.java`](java/SynthTerminal.java) | see the Java block below |
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
