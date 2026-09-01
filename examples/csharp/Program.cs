// A runnable C# example: drive a synthetic feed and print a frame.
//
//   cargo build --release -p wickra-terminal-c
//   dotnet run --project examples/csharp
//
// A second scenario, the time-machine, is selected by argument:
//
//   dotnet run --project examples/csharp -- time-machine
//
// One project rather than a directory per example: the csproj carries forty
// lines of native-library plumbing, and a copy of it per scenario is a copy
// that goes stale.
using WickraTerminal;

if (args.Length > 0 && args[0] == "time-machine")
{
    TimeMachine.Run();
    return;
}

const string config =
    "{\"sources\":[{\"Synth\":{\"seed\":1}}]," +
    "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

using var term = new Terminal(config);
term.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");

string raw = string.Empty;
for (int i = 0; i < 20; i++)
{
    raw = term.Command("{\"type\":\"Tick\"}");
}

Console.WriteLine($"wickra-terminal {Terminal.Version()}");
Console.WriteLine(raw);
