# Wickra Terminal — C# / .NET

.NET bindings for the `wickra-terminal` data-driven core, over the Wickra C ABI
(P/Invoke). Build a `Terminal` from a JSON config, drive it with command JSON,
read back frame view-models — the same protocol as the native TUI and every
other binding.

## Install

```bash
dotnet add package WickraTerminal
```

## Usage

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

## Build and test from source

```bash
cargo build -p wickra-terminal-c          # produces the native wickra_terminal library
dotnet test bindings/csharp/WickraTerminal.Tests
```

The test project copies the native library from `target/debug/` next to the test
assembly so the default P/Invoke resolver finds it.
