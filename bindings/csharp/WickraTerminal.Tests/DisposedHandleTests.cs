using Xunit;

namespace WickraTerminal.Tests;

/// <summary>
/// The terminal used to hold a raw <c>IntPtr</c> with a class finalizer, which
/// is the pattern <c>SafeHandle</c> exists to replace. Two holes came with it:
/// the collector could finalize the terminal — and so free it — while a call
/// that had already read the pointer into a register was still running, and
/// <c>Dispose</c> was an unsynchronised read-call-write that two threads could
/// both enter and free twice.
///
/// It now owns a <see cref="TerminalHandle"/>, so the runtime ref-counts the
/// handle across every call and releases it exactly once.
/// </summary>
public class DisposedHandleTests
{
    private const string Config =
        "{\"sources\":[{\"Synth\":{\"seed\":1}}]," +
        "\"layout\":{\"panels\":[{\"kind\":\"Chart\",\"rect\":{\"x\":0,\"y\":0,\"w\":100,\"h\":100}}]}}";

    [Fact]
    public void CommandAfterDisposeThrows()
    {
        var term = new Terminal(Config);
        term.Command("{\"type\":\"Tick\"}");
        term.Dispose();

        Assert.Throws<ObjectDisposedException>(() => term.Command("{\"type\":\"Tick\"}"));
    }

    [Fact]
    public void DisposeIsIdempotent()
    {
        var term = new Terminal(Config);
        term.Dispose();
        term.Dispose();
    }

    [Fact]
    public void ConcurrentDisposeReleasesOnce()
    {
        // The double-free the old Dispose allowed: eight threads racing on a read
        // of the pointer, a free and a write. A double free is an access
        // violation that takes the test host with it, so this failing is not a
        // red test — it is no test process at all.
        for (int attempt = 0; attempt < 200; attempt++)
        {
            var term = new Terminal(Config);
            var threads = new Thread[8];
            for (int i = 0; i < threads.Length; i++)
            {
                threads[i] = new Thread(term.Dispose);
            }
            foreach (var thread in threads)
            {
                thread.Start();
            }
            foreach (var thread in threads)
            {
                thread.Join();
            }
        }
    }

    [Fact]
    public void AnUndisposedTerminalIsUnaffected()
    {
        // The guard must not disturb the normal path.
        using var term = new Terminal(Config);
        term.Command("{\"type\":\"Subscribe\",\"source\":0,\"symbol\":\"BTC/USDT\"}");
        string frame = term.Command("{\"type\":\"Tick\"}");
        Assert.Contains("panels", frame);
    }

    [Fact]
    public void AnInvalidConfigDoesNotLeaveAHandleBehind()
    {
        Assert.Throws<ArgumentException>(() => new Terminal("{\"sources\":"));
    }
}
