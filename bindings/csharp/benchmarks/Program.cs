// Throughput benchmark for the wickra-terminal C# binding.
//
// What this measures is the boundary, not the core. Every binding drives the
// same Rust terminal through one function -- a command JSON in, a frame JSON
// out -- so the number is the cost of crossing this boundary once per command.
// The C# row includes marshalling two strings and the SafeHandle ref-count the
// binding takes on every call, which is what makes a call safe against a
// concurrent dispose.
//
//   cargo build -p wickra-terminal-c --release
//   dotnet run --project bindings/csharp/benchmarks -c Release
//   dotnet run --project bindings/csharp/benchmarks -c Release -- 100000

using System.Diagnostics;
using System.Globalization;
using System.Text;
using WickraTerminal;

// Shared by all nine binding benchmarks, so the numbers compare.
const string Config =
    """{"sources":[{"Synth":{"seed":1}}],"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":40}},{"kind":"Book","rect":{"x":0,"y":40,"w":50,"h":30}},{"kind":"Tape","rect":{"x":50,"y":40,"w":50,"h":30}}]}}""";
const string Subscribe = """{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}""";
const string Tick = """{"type":"Tick"}""";
const string ListCommand = """{"type":"ListIndicators"}""";

// The catalogue response is ~30 kB, so a hundred of them is a noisy sample.
const int CatalogueReps = 1000;

int ticks = args.Length > 0 && int.TryParse(args[0], out int parsed) && parsed >= 100 ? parsed : 20_000;

using var term = new Terminal(Config);
term.Command(Subscribe);
int frameBytes = Encoding.UTF8.GetByteCount(term.Command(Tick));
int catalogueBytes = Encoding.UTF8.GetByteCount(term.Command(ListCommand));

// Median of three timed runs, after one warmup.
double MedianNs(string command, int count)
{
    void Drive()
    {
        for (int i = 0; i < count; i++)
        {
            term.Command(command);
        }
    }

    Drive();
    double[] samples = new double[3];
    for (int i = 0; i < samples.Length; i++)
    {
        var watch = Stopwatch.StartNew();
        Drive();
        watch.Stop();
        samples[i] = watch.Elapsed.TotalMilliseconds * 1e6;
    }
    Array.Sort(samples);
    return samples[1];
}

double tickNs = MedianNs(Tick, ticks);
double listNs = MedianNs(ListCommand, CatalogueReps);

var culture = CultureInfo.InvariantCulture;
string Rate(double count, double ns) => (count / (ns / 1e9)).ToString("N0", culture);
string Micros(double ns, double count) => (ns / count / 1e3).ToString("F2", culture);
string Bytes(int value) => value.ToString("N0", culture) + "B";

Console.WriteLine($"wickra-terminal C# throughput - {ticks.ToString("N0", culture)} commands (median of 3)\n");
Console.WriteLine($"{"Command",-18}{"per second",14}{"us/command",14}{"payload",12}");
Console.WriteLine(new string('-', 58));
Console.WriteLine($"{"Tick",-18}{Rate(ticks, tickNs),14}{Micros(tickNs, ticks),14}{Bytes(frameBytes),12}");
Console.WriteLine($"{"ListIndicators",-18}{Rate(CatalogueReps, listNs),14}{Micros(listNs, CatalogueReps),14}{Bytes(catalogueBytes),12}");
Console.WriteLine("\nOne command crosses the boundary once. Higher is better, and the numbers\n"
    + "are machine-dependent -- compare bindings on one machine, never across two.");
