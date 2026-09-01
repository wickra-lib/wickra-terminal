using System.Text;
using System.Text.Json;
using WickraTerminal;

/// <summary>
/// The time-machine scenario: rewind a recorded feed and watch state re-fold.
///
/// <c>Seek</c> throws the folded state away and rebuilds it from the recording,
/// so a rewind is deterministic rather than approximate — which is what makes a
/// recording more than a slow synthetic feed. Nothing here is C#-specific: it is
/// four JSON commands, and every binding drives the same four.
/// </summary>
internal static class TimeMachine
{
    private const int Trades = 6;

    /// <summary>The recorded feed, as the JSON array a Replay source takes.</summary>
    private static string Config()
    {
        var events = new StringBuilder("[");
        for (int i = 0; i < Trades; i++)
        {
            if (i > 0)
            {
                events.Append(',');
            }
            events.Append("{\"type\":\"trade\",\"symbol\":{\"base\":\"BTC\",\"quote\":\"USDT\"},")
                  .Append("\"price\":\"").Append(100 + i).Append("\",\"quantity\":\"1\",")
                  .Append("\"aggressor\":\"Buy\",\"timestamp\":").Append(i + 1).Append('}');
        }
        events.Append(']');
        string feed = JsonSerializer.Serialize(events.ToString());
        return "{\"sources\":[{\"Replay\":{\"dataset\":" + feed + "}}]," +
               "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";
    }

    /// <summary>The chart panel's last price, out of a frame.</summary>
    private static double LastPrice(string frame)
    {
        using var parsed = JsonDocument.Parse(frame);
        foreach (var panel in parsed.RootElement.GetProperty("panels").EnumerateArray())
        {
            if (panel.GetProperty("panel").GetString() == "chart")
            {
                return panel.GetProperty("last").GetDouble();
            }
        }
        return 0;
    }

    public static void Run()
    {
        using var term = new Terminal(Config());
        term.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");

        string raw = string.Empty;
        for (int i = 0; i < Trades; i++)
        {
            raw = term.Command("{\"type\":\"Tick\"}");
        }
        Console.WriteLine($"played to the end:   last = {LastPrice(raw)}");

        Console.WriteLine("position:            " +
            term.Command("{\"type\":\"ReplayPosition\",\"source\":0}"));

        // Rewind to just after the second trade. The state is rebuilt from the
        // recording rather than restored from a snapshot, which is why a rewind
        // lands on exactly the frame the forward pass had at that point.
        raw = term.Command("{\"type\":\"Seek\",\"source\":0,\"index\":2}");
        Console.WriteLine($"rewound to index 2:  last = {LastPrice(raw)}");

        // And forward again from there, over the same events.
        raw = term.Command("{\"type\":\"Tick\"}");
        Console.WriteLine($"one tick later:      last = {LastPrice(raw)}");
    }
}
