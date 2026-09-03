using System.Text.Json;
using Xunit;

namespace WickraTerminal.Tests;

/// <summary>
/// The recorder, the scrubber and the host feed, end to end through the binding.
///
/// Four commands sit on the boundary, are documented in all nine binding
/// READMEs, and were driven by almost no binding: <c>SetRecording</c> and
/// <c>ExportRecording</c> by none at all, <c>ReplayPosition</c> only by the C
/// example, <c>FeedDerivatives</c> by none. The README completeness test proved
/// the promise and nothing checked it was kept, so the recorder had never been
/// executed outside Rust.
///
/// The round trip is the point: arm the recorder, drive the terminal, export
/// what it kept, and hand that straight back as a <c>Replay</c> dataset. A
/// binding that mangled the export would be caught by the replay refusing it,
/// which no assertion about a string shape would find.
/// </summary>
public class RecorderTests
{
    // A derivatives indicator, so FeedDerivatives is observable in the frame
    // rather than merely accepted.
    private const string Config =
        "{\"sources\":[\"Manual\"]," +
        "\"indicators\":[{\"kind\":\"FundingRate\",\"params\":[]}]," +
        "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

    private const string Symbol = "BTC/USDT";

    private static Terminal Subscribed()
    {
        var term = new Terminal(Config);
        term.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"" + Symbol + "\"}");
        return term;
    }

    private static string ReplayConfig(string dataset) =>
        "{\"sources\":[{\"Replay\":{\"dataset\":" + JsonSerializer.Serialize(dataset) + "}}]," +
        "\"indicators\":[]," +
        "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

    private static string Drive(Terminal term, string price, int timestamp)
    {
        term.Command(
            "{\"type\":\"Feed\",\"source\":0,\"event\":{\"type\":\"trade\"," +
            "\"symbol\":{\"base\":\"BTC\",\"quote\":\"USDT\"},\"price\":\"" + price + "\"," +
            "\"quantity\":\"0.5\",\"aggressor\":\"Buy\",\"timestamp\":" + timestamp + "}}");
        return term.Command("{\"type\":\"Tick\"}");
    }

    private static JsonElement Chart(string raw)
    {
        foreach (var panel in JsonDocument.Parse(raw).RootElement.GetProperty("panels").EnumerateArray())
        {
            if (panel.GetProperty("panel").GetString() == "chart")
            {
                return panel.Clone();
            }
        }

        throw new Xunit.Sdk.XunitException("no chart panel in " + raw);
    }

    [Fact]
    public void Recorder_RoundTripsThroughAReplay()
    {
        using var term = Subscribed();

        // Nothing is kept until the recorder is armed, and asking is not an error.
        Assert.Equal("[]", term.Command("{\"type\":\"ExportRecording\"}"));

        term.Command("{\"type\":\"SetRecording\",\"capacity\":64}");
        var prices = new[] { "100", "101", "102", "103" };
        for (int i = 0; i < prices.Length; i++)
        {
            Drive(term, prices[i], i + 1);
        }

        var recording = term.Command("{\"type\":\"ExportRecording\"}");
        var events = JsonDocument.Parse(recording).RootElement;
        Assert.Equal(4, events.GetArrayLength());
        Assert.Equal("100", events[0].GetProperty("price").GetString());
        Assert.Equal("103", events[3].GetProperty("price").GetString());

        // Straight back in as a dataset: the shape Replay takes is the shape
        // ExportRecording answers with, which is what makes a session keepable.
        using var replay = new Terminal(ReplayConfig(recording));
        replay.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"" + Symbol + "\"}");
        var raw = string.Empty;
        for (int i = 0; i < 4; i++)
        {
            raw = replay.Command("{\"type\":\"Tick\"}");
        }

        Assert.Equal(103.0, Chart(raw).GetProperty("last").GetDouble());
    }

    [Fact]
    public void StoppingTheRecorder_ClearsWhatItHeld()
    {
        // Both directions clear, so a capacity change never leaves a recording
        // that is part one size and part another.
        using var term = Subscribed();
        term.Command("{\"type\":\"SetRecording\",\"capacity\":64}");
        Drive(term, "100", 1);
        Assert.NotEqual("[]", term.Command("{\"type\":\"ExportRecording\"}"));

        term.Command("{\"type\":\"SetRecording\",\"capacity\":null}");
        Assert.Equal("[]", term.Command("{\"type\":\"ExportRecording\"}"));
    }

    [Fact]
    public void ReplayPosition_AnswersForASourceThatCannotBeReplayed()
    {
        // 0/0 rather than an error, so a renderer can ask about whatever is
        // focused without first knowing what kind of source it is.
        using var term = Subscribed();
        var raw = term.Command("{\"type\":\"ReplayPosition\",\"source\":0}");
        var where = JsonDocument.Parse(raw).RootElement;
        Assert.Equal(0, where.GetProperty("cursor").GetInt32());
        Assert.Equal(0, where.GetProperty("length").GetInt32());
    }

    [Fact]
    public void ReplayPosition_TracksTheCursorThroughARecording()
    {
        using var term = Subscribed();
        term.Command("{\"type\":\"SetRecording\",\"capacity\":64}");
        var prices = new[] { "100", "101", "102", "103" };
        for (int i = 0; i < prices.Length; i++)
        {
            Drive(term, prices[i], i + 1);
        }

        using var replay = new Terminal(ReplayConfig(term.Command("{\"type\":\"ExportRecording\"}")));
        replay.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"" + Symbol + "\"}");

        var start = JsonDocument.Parse(
            replay.Command("{\"type\":\"ReplayPosition\",\"source\":0}")).RootElement;
        Assert.Equal(0, start.GetProperty("cursor").GetInt32());
        Assert.Equal(4, start.GetProperty("length").GetInt32());

        for (int i = 0; i < 3; i++)
        {
            replay.Command("{\"type\":\"Tick\"}");
        }

        var moved = JsonDocument.Parse(
            replay.Command("{\"type\":\"ReplayPosition\",\"source\":0}")).RootElement;
        Assert.Equal(3, moved.GetProperty("cursor").GetInt32());
        Assert.Equal(4, moved.GetProperty("length").GetInt32());
    }

    [Fact]
    public void FedDerivatives_ReachADerivativesIndicator()
    {
        // Accepting the command proves nothing on its own: the update is folded
        // into the market's microstructure and reaches an indicator only on the
        // next trade, so the reading is what says it arrived.
        using var term = Subscribed();
        var before = Chart(Drive(term, "100", 1)).GetProperty("indicators")[0];
        Assert.Equal(JsonValueKind.Null, before.GetProperty("value").ValueKind);

        // All three prices, or the tick is withheld: a mark without an index and
        // a futures price is not a priced market.
        term.Command(
            "{\"type\":\"FeedDerivatives\",\"source\":0,\"symbol\":\"" + Symbol + "\"," +
            "\"update\":{\"funding_rate\":0.0001,\"mark_price\":102.0,\"index_price\":100.0," +
            "\"futures_price\":104.0,\"open_interest\":1000.0,\"timestamp\":9}}");

        var reading = Chart(Drive(term, "101", 2)).GetProperty("indicators")[0];
        Assert.Equal("FundingRate", reading.GetProperty("name").GetString());
        Assert.Equal(0.0001, reading.GetProperty("value").GetDouble(), 12);
    }

    [Fact]
    public void FeedingDerivativesToAnUntrackedMarket_Throws()
    {
        using var term = new Terminal(Config);
        Assert.Throws<InvalidOperationException>(() => term.Command(
            "{\"type\":\"FeedDerivatives\",\"source\":0,\"symbol\":\"" + Symbol + "\"," +
            "\"update\":{\"funding_rate\":0.0001,\"timestamp\":1}}"));
    }
}
