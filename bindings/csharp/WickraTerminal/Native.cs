using System.Runtime.InteropServices;

namespace WickraTerminal;

/// <summary>Raw P/Invoke surface for the wickra-terminal C ABI.</summary>
internal static class Native
{
    private const string Lib = "wickra_terminal";

    public const int Ok = 0;
    public const int ErrNull = -1;
    public const int Err = -2;

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr wickra_terminal_new(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string configJson);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void wickra_terminal_free(IntPtr handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern int wickra_terminal_command(
        IntPtr handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string cmdJson,
        out IntPtr outJson);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void wickra_terminal_free_string(IntPtr s);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr wickra_terminal_version();
}
