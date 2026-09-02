using System.Text;
using System.Text.Json;
using Xunit;

namespace WickraTerminal.Tests;

/// <summary>
/// Streaming a feed and re-folding it in one batch reach the same frame.
///
/// The terminal reaches a state two ways. Streaming folds one event per tick as
/// it arrives; <c>Seek</c> throws the state away and re-folds the whole prefix
/// in a single batch. ARCHITECTURE.md calls that re-fold the moat -- it is what
/// makes a rewind deterministic and what lets the browser run the time-machine
/// with no engine behind it -- so the two must land on byte-identical frames.
///
/// Byte-identical, not merely equal: the binding returns the core's compact
/// command output verbatim, so string equality here is the exact check with no
/// JSON comparison in the way. The Rust suite proves the core re-folds
/// correctly; this proves the binding carries the same bytes out.
/// </summary>
public class StreamingTests
{
    private const int Ticks = 4;
    private const int Events = 8;

    private static string Config()
    {
        var events = new StringBuilder("[");
        for (int i = 0; i < Events; i++)
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
        // Serialised as a JSON string, because `dataset` carries the feed as one.
        string feed = JsonSerializer.Serialize(events.ToString());
        return "{\"sources\":[{\"Replay\":{\"dataset\":" + feed + "}}]," +
               "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";
    }

    private static Terminal Subscribed()
    {
        var term = new Terminal(Config());
        term.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
        return term;
    }

    [Fact]
    public void StreamingAndBatchRefold_Agree()
    {
        using var streamed = Subscribed();
        string frame = string.Empty;
        for (int i = 0; i < Ticks; i++)
        {
            frame = streamed.Command("{\"type\":\"Tick\"}");
        }

        // A second terminal runs the feed out, then re-folds the same prefix in
        // one batch. Running past the point first is what makes this a rewind
        // rather than a replay of state it still had.
        using var rewound = Subscribed();
        for (int i = 0; i < Events; i++)
        {
            rewound.Command("{\"type\":\"Tick\"}");
        }
        string refolded = rewound.Command(
            "{\"type\":\"Seek\",\"source\":0,\"index\":" + Ticks + "}");

        Assert.Equal(frame, refolded);
    }

    [Fact]
    public void TheComparedFrame_IsNotEmpty()
    {
        // A guard on the guard: two empty frames are also byte-identical, and an
        // equality test that passes on nothing proves nothing.
        using var term = Subscribed();
        string raw = string.Empty;
        for (int i = 0; i < Ticks; i++)
        {
            raw = term.Command("{\"type\":\"Tick\"}");
        }
        Assert.Contains("\"last\":" + (100 + Ticks - 1), raw);
    }
}
