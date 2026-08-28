using System.Runtime.InteropServices;

namespace WickraTerminal;

/// <summary>
/// A trading-terminal instance driven by JSON commands, over the Wickra C ABI.
/// Build one from a JSON config, drive it with command JSON and read back the
/// frame JSON — the same protocol as the native TUI and every other binding.
/// </summary>
public sealed class Terminal : IDisposable
{
    private readonly TerminalHandle _handle;

    /// <summary>Build a terminal from a JSON config string.</summary>
    /// <exception cref="ArgumentException">The config is null/invalid.</exception>
    public Terminal(string configJson)
    {
        _handle = Native.wickra_terminal_new(configJson);
        if (_handle.IsInvalid)
        {
            _handle.Dispose();
            throw new ArgumentException("wickra-terminal: invalid config", nameof(configJson));
        }
    }

    /// <summary>Apply a command JSON and return the resulting frame JSON.</summary>
    /// <exception cref="InvalidOperationException">The command failed.</exception>
    public string Command(string cmdJson)
    {
        // The marshaller would raise this too, from the handle rather than from
        // the terminal; checking here names the type the caller actually holds.
        ObjectDisposedException.ThrowIf(_handle.IsClosed, this);

        int code = Native.wickra_terminal_command(_handle, cmdJson, out IntPtr outPtr);
        string result = outPtr == IntPtr.Zero ? string.Empty : Marshal.PtrToStringUTF8(outPtr) ?? string.Empty;
        if (outPtr != IntPtr.Zero)
        {
            Native.wickra_terminal_free_string(outPtr);
        }
        if (code != Native.Ok)
        {
            throw new InvalidOperationException($"wickra-terminal: {result}");
        }
        return result;
    }

    /// <summary>The library version.</summary>
    public static string Version() =>
        Marshal.PtrToStringUTF8(Native.wickra_terminal_version()) ?? string.Empty;

    /// <summary>
    /// Free the native terminal handle. Idempotent, and safe to call from more
    /// than one thread: the handle releases once, under the runtime's ref count.
    /// </summary>
    /// <remarks>
    /// No finalizer here. <see cref="TerminalHandle"/> carries a critical one, so
    /// a terminal that is never disposed is still released, and a call in flight
    /// cannot have the handle freed underneath it.
    /// </remarks>
    public void Dispose() => _handle.Dispose();
}
