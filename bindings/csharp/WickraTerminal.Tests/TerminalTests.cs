using Xunit;

namespace WickraTerminal.Tests;

public class TerminalTests
{
    private const string Config =
        "{\"sources\":[{\"Synth\":{\"seed\":1}}]," +
        "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

    [Fact]
    public void Version_IsNonEmpty()
    {
        Assert.False(string.IsNullOrEmpty(Terminal.Version()));
    }

    [Fact]
    public void SubscribeThenTick_ReturnsChartFrame()
    {
        using var term = new Terminal(Config);
        term.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
        string raw = string.Empty;
        for (int i = 0; i < 30; i++)
        {
            raw = term.Command("{\"type\":\"Tick\"}");
        }
        Assert.Contains("\"panel\":\"chart\"", raw);
    }

    [Fact]
    public void InvalidConfig_Throws()
    {
        Assert.Throws<ArgumentException>(() => new Terminal("not json"));
    }

    [Fact]
    public void InvalidCommand_Throws()
    {
        using var term = new Terminal(Config);
        Assert.Throws<InvalidOperationException>(() => term.Command("{\"type\":\"Nope\"}"));
    }
}
