using System.Text.Json;
using Xunit;

namespace WickraTerminal.Tests;

/// <summary>
/// Cross-language golden parity, driven by <c>golden/manifest.json</c>.
///
/// Each scenario names a config and a command sequence; replaying it must produce
/// the frame in its expected file, byte for byte. Because the binding returns the
/// core's compact <c>command_json</c> string verbatim, byte equality against that
/// one file is the exact parity check.
///
/// Reading the manifest rather than naming one scenario is what makes the corpus
/// extensible: a scenario added in the Rust suite is picked up here, and in the
/// seven other language suites, with no change to any of them.
/// </summary>
public class GoldenTests
{
    private sealed record Scenario(string Name, string Config, string Expected, string Commands);

    private static string GoldenDir()
    {
        var dir = AppContext.BaseDirectory;
        for (int i = 0; i < 10; i++)
        {
            var candidate = Path.Combine(dir, "golden");
            if (File.Exists(Path.Combine(candidate, "manifest.json")))
            {
                return candidate;
            }
            var parent = Directory.GetParent(dir);
            if (parent is null)
            {
                break;
            }
            dir = parent.FullName;
        }
        throw new DirectoryNotFoundException("golden/ not found");
    }

    private static (string Dir, List<Scenario> Scenarios) Manifest()
    {
        var dir = GoldenDir();
        using var doc = JsonDocument.Parse(File.ReadAllText(Path.Combine(dir, "manifest.json")));
        var scenarios = new List<Scenario>();
        foreach (var entry in doc.RootElement.GetProperty("scenarios").EnumerateArray())
        {
            scenarios.Add(new Scenario(
                entry.GetProperty("name").GetString()!,
                entry.GetProperty("config").GetString()!,
                entry.GetProperty("expected").GetString()!,
                entry.GetProperty("commands").GetString()!));
        }
        return (dir, scenarios);
    }

    public static TheoryData<string> ScenarioNames()
    {
        var data = new TheoryData<string>();
        foreach (var scenario in Manifest().Scenarios)
        {
            data.Add(scenario.Name);
        }
        return data;
    }

    [Theory]
    [MemberData(nameof(ScenarioNames))]
    public void GoldenParity_FrameIsByteExact(string name)
    {
        var (dir, scenarios) = Manifest();
        var scenario = scenarios.Single(s => s.Name == name);

        var config = File.ReadAllText(Path.Combine(dir, scenario.Config.Replace('/', Path.DirectorySeparatorChar)));
        var expected = File.ReadAllText(Path.Combine(dir, scenario.Expected.Replace('/', Path.DirectorySeparatorChar))).Trim();

        var commands = File
            .ReadAllLines(Path.Combine(dir, scenario.Commands.Replace('/', Path.DirectorySeparatorChar)))
            .Where(line => !string.IsNullOrWhiteSpace(line))
            .ToArray();
        Assert.NotEmpty(commands);

        using var term = new Terminal(config);
        var frame = string.Empty;
        foreach (var command in commands)
        {
            frame = term.Command(command);
        }
        Assert.Equal(expected, frame.Trim());
    }

    [Fact]
    public void Corpus_CoversMoreThanOneScenario()
    {
        // A manifest that silently shrank to one entry would leave every parity
        // test passing while checking a fraction of what it used to.
        var names = Manifest().Scenarios.Select(s => s.Name).ToList();
        Assert.True(names.Count >= 9, $"only {names.Count} scenarios");
        foreach (var expected in new[] { "basic", "book_deltas", "footprint", "indicators", "seek" })
        {
            Assert.Contains(expected, names);
        }
    }
}
