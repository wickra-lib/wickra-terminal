using System.Text.Json;
using Xunit;

namespace WickraTerminal.Tests;

/// <summary>
/// The indicator registry is reachable from .NET.
///
/// The registry lives in the Rust core and this binding passes JSON through the
/// C ABI, so nothing here needed new binding code. That is exactly why it is
/// worth a test: "no code changed" is also what a broken pass-through looks like.
/// </summary>
public class RegistryTests
{
    // A non-default indicator, so finding it proves the config reached the
    // registry rather than the built-in overlay happening to look right.
    private const string Config =
        "{\"sources\":[{\"Synth\":{\"seed\":1}}]," +
        "\"indicators\":[{\"kind\":\"Rsi\",\"params\":[14]}]}";

    private const string Tick = "{\"type\":\"Tick\"}";

    private static Terminal Subscribed()
    {
        var term = new Terminal(Config);
        term.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
        return term;
    }

    private static JsonElement ChartPanel(string frameJson)
    {
        using var doc = JsonDocument.Parse(frameJson);
        foreach (var panel in doc.RootElement.GetProperty("panels").EnumerateArray())
        {
            if (panel.GetProperty("panel").GetString() == "chart")
            {
                return panel.Clone();
            }
        }
        throw new Xunit.Sdk.XunitException("no chart panel in the frame");
    }

    private static List<string> ChartIndicators(Terminal term)
    {
        var chart = ChartPanel(term.Command(Tick));
        var names = new List<string>();
        foreach (var indicator in chart.GetProperty("indicators").EnumerateArray())
        {
            names.Add(indicator.GetProperty("name").GetString()!);
        }
        return names;
    }

    [Fact]
    public void ConfiguredIndicator_ReachesTheChart()
    {
        using var term = Subscribed();
        var names = new List<string>();
        for (int i = 0; i < 30; i++)
        {
            names = ChartIndicators(term);
        }
        Assert.Equal(new[] { "Rsi(14)" }, names);
    }

    [Fact]
    public void Indicators_CanBeAddedAndRemovedAtRunTime()
    {
        using var term = Subscribed();
        term.Command("{\"type\":\"AddIndicator\",\"spec\":{\"kind\":\"Atr\",\"params\":[14]}}");
        Assert.Contains("Atr(14)", ChartIndicators(term));
        term.Command("{\"type\":\"RemoveIndicator\",\"label\":\"Rsi(14)\"}");
        Assert.Equal(new[] { "Atr(14)" }, ChartIndicators(term));
    }

    [Fact]
    public void Catalogue_ListsTheWholeRegistry()
    {
        using var term = Subscribed();
        using var doc = JsonDocument.Parse(term.Command("{\"type\":\"ListIndicators\"}"));
        var rows = doc.RootElement.GetProperty("indicators");
        Assert.True(rows.GetArrayLength() >= 421, $"only {rows.GetArrayLength()} entries");

        // Every row carries the parameters needed to construct it: discovery
        // without a second lookup.
        var byKind = new Dictionary<string, int>();
        foreach (var row in rows.EnumerateArray())
        {
            byKind[row.GetProperty("kind").GetString()!] =
                row.GetProperty("params").GetArrayLength();
        }
        Assert.Equal(1, byKind["Sma"]);
        Assert.Equal(3, byKind["MacdIndicator"]);
        Assert.Equal(0, byKind["AdaptiveCycle"]);
    }

    [Fact]
    public void UnknownIndicator_IsRejectedWithItsName()
    {
        using var term = Subscribed();
        var err = Assert.ThrowsAny<Exception>(() =>
            term.Command("{\"type\":\"AddIndicator\",\"spec\":{\"kind\":\"NotReal\"}}"));
        Assert.Contains("NotReal", err.Message);
    }

    [Fact]
    public void MultiOutputIndicator_ReportsNamedFields()
    {
        using var term = new Terminal(
            "{\"sources\":[{\"Synth\":{\"seed\":1}}]," +
            "\"indicators\":[{\"kind\":\"MacdIndicator\",\"params\":[12,26,9]}]}");
        term.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
        string raw = string.Empty;
        for (int i = 0; i < 200; i++)
        {
            raw = term.Command(Tick);
        }
        var chart = ChartPanel(raw);
        var macd = chart.GetProperty("indicators")[0];
        Assert.Equal("MacdIndicator(12,26,9)", macd.GetProperty("name").GetString());

        var fields = macd.GetProperty("fields");
        Assert.True(fields.GetArrayLength() > 1, "expected more than one named field");
        // The primary value is the first field, so a caller wanting one line
        // does not have to know which field that is.
        Assert.Equal(
            macd.GetProperty("value").GetDouble(),
            fields[0].GetProperty("value").GetDouble());
    }
}
