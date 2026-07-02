using System;
using System.IO;
using Xunit;

namespace WickraTerminal.Tests;

// Cross-language golden parity: build the terminal from the committed
// golden/config.json, replay the feed, and assert the frame equals
// golden/expected/basic.min.json byte-for-byte. The binding returns the core's
// compact command_json string verbatim, so byte equality against that one file is
// the exact cross-language parity check.
public class GoldenTests
{
    private static string GoldenDir()
    {
        string? dir = AppContext.BaseDirectory;
        for (int i = 0; i < 12 && dir is not null; i++)
        {
            string g = Path.Combine(dir, "golden");
            if (File.Exists(Path.Combine(g, "config.json")))
            {
                return g;
            }
            dir = Path.GetDirectoryName(dir);
        }
        throw new InvalidOperationException("golden/ not found");
    }

    [Fact]
    public void GoldenParity_FrameIsByteExact()
    {
        string g = GoldenDir();
        string config = File.ReadAllText(Path.Combine(g, "config.json"));
        string expected = File.ReadAllText(Path.Combine(g, "expected", "basic.min.json")).Trim();

        using var term = new Terminal(config);
        term.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
        string frame = string.Empty;
        for (int i = 0; i < 32; i++)
        {
            frame = term.Command("{\"type\":\"Tick\"}");
        }
        Assert.Equal(expected, frame.Trim());
    }
}
