# Wickra Terminal — C / C++ examples

The Wickra Terminal C ABI is a single shared/static library plus a generated header
([`bindings/c/include/wickra_terminal.h`](../../bindings/c/include/wickra_terminal.h)). Any C-capable
language links against the same artifact; these examples show the plain-C path
and, through [`wickra_terminal.hpp`](../../bindings/c/include/wickra_terminal.hpp), the C++ one.

## Build the library

From the workspace root:

```sh
cargo build -p wickra-terminal-c --release
```

This produces, in `target/release/`:

| Platform | Shared library | Link target |
|----------|----------------|-------------|
| Linux    | `libwickra_terminal.so`     | `-lwickra_terminal` |
| macOS    | `libwickra_terminal.dylib`  | `-lwickra_terminal` |
| Windows (MSVC) | `wickra_terminal.dll` | `wickra_terminal.dll.lib` (import lib) |

A static library (`libwickra_terminal.a` / `wickra_terminal.lib`) is emitted alongside.

## Build and run the examples

### With CMake (portable, used by CI)

```sh
cmake -S examples/c -B examples/c/build
cmake --build examples/c/build --config Release
ctest --test-dir examples/c/build -C Release --output-on-failure
```

### Directly with a compiler

```sh
# Linux / macOS
cc examples/c/synth.c -I bindings/c/include -L target/release -lwickra_terminal -lm -o synth
LD_LIBRARY_PATH=target/release ./synth        # macOS: DYLD_LIBRARY_PATH

# Windows (MinGW gcc, linking the DLL directly)
gcc examples/c/synth.c -I bindings/c/include target/release/wickra_terminal.dll -lm -o synth.exe
```

## The examples

| Example | What it does |
|---------|--------------|
| `lifetime.cpp` | Lifetime tests for the C++ RAII header. |
| `synth.c` | A minimal C example: drive a synthetic feed and print a frame. |
| `terminal.cpp` | A C++ example, over the shipped RAII header. |
| `time_machine.c` | A runnable C example: rewind a recorded feed and watch state re-fold. |

## Usage shape

Every call follows the same handle discipline: construct from a spec JSON, drive
with command JSON, read the response, free the handle exactly once. `wickra_terminal.h` is
the whole contract; the C++ header, where one ships, wraps the handle in a
move-only RAII type. See [`bindings/c/README.md`](../../bindings/c/README.md).
