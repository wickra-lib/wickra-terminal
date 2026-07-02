# WickraTerminal

The data-driven trading-terminal core for .NET, over the Wickra C ABI. Build a
`Terminal` from a JSON config, drive it with command JSON, read back frame
view-models — the same protocol as the native TUI and every other binding.

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

The native `wickra_terminal` library is packaged per runtime identifier under
`runtimes/<rid>/native/`.
