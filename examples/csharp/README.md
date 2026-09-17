# Wickra Terminal examples — C#

Runnable C# examples for the [Wickra Terminal C# binding](../../bindings/csharp). The binding consumes the C ABI
library through P/Invoke, so build it once before running anything:

```bash
cargo build -p wickra-terminal-c --release
```

## Run

As the CI examples job runs it, from the repository root:

```bash
dotnet run --project examples/csharp/<Example>
```

## The examples

| Example | What it does |
|---------|--------------|
| `Program.cs` | A runnable C# example: drive a synthetic feed and print a frame. |
| `TimeMachine.cs` | A runnable example against this binding. |
