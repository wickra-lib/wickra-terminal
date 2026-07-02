using System.Runtime.InteropServices;

namespace WickraTerminal;

/// <summary>
/// A trading-terminal instance driven by JSON commands, over the Wickra C ABI.
/// Build one from a JSON config, drive it with command JSON and read back the
/// frame JSON — the same protocol as the native TUI and every other binding.
/// </summary>
public sealed class Terminal : IDisposable
{
    private IntPtr _handle;

    /// <summary>Build a terminal from a JSON config string.</summary>
    /// <exception cref="ArgumentException">The config is null/invalid.</exception>
    public Terminal(string configJson)
    {
        _handle = Native.wickra_terminal_new(configJson);
        if (_handle == IntPtr.Zero)
        {
            throw new ArgumentException("wickra-terminal: invalid config", nameof(configJson));
        }
    }

    /// <summary>Apply a command JSON and return the resulting frame JSON.</summary>
    /// <exception cref="InvalidOperationException">The command failed.</exception>
    public string Command(string cmdJson)
    {
        ObjectDisposedException.ThrowIf(_handle == IntPtr.Zero, this);

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

    /// <summary>Free the native terminal handle.</summary>
    public void Dispose()
    {
        if (_handle != IntPtr.Zero)
        {
            Native.wickra_terminal_free(_handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    /// <summary>Finalizer — frees the handle if Dispose was not called.</summary>
    ~Terminal() => Dispose();
}
