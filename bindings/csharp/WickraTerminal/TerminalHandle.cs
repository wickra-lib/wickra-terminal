using Microsoft.Win32.SafeHandles;

namespace WickraTerminal;

/// <summary>
/// Owns the opaque native terminal handle and releases it through
/// <c>wickra_terminal_free</c>.
/// </summary>
/// <remarks>
/// <para>
/// A <see cref="System.Runtime.InteropServices.SafeHandle"/> rather than a raw
/// <c>IntPtr</c> with a finalizer, which is exactly the pattern SafeHandle
/// exists to replace. The raw form had two holes. The collector may finalize a
/// <c>Terminal</c> once nothing refers to it, and reading <c>_handle</c> into an
/// argument register is the last reference
/// — so the finalizer could free the terminal while the call using it was still
/// running. And <c>Dispose</c> was a read, a call and a write with no
/// synchronisation, so two threads disposing at once could both see a live
/// handle and free it twice.
/// </para>
/// <para>
/// The marshaller ref-counts a SafeHandle across every call it is passed to, so
/// it cannot be released mid-call and cannot be released twice, and it raises
/// <see cref="ObjectDisposedException"/> on a call after disposal instead of
/// reaching freed memory. Its release also runs in a critical finalizer, which
/// the runtime will not skip. This is the shape the wickra library's
/// <c>WickraHandle</c> already had.
/// </para>
/// </remarks>
internal sealed class TerminalHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    /// <summary>Constructed by the marshaller for a returned handle.</summary>
    internal TerminalHandle()
        : base(ownsHandle: true)
    {
    }

    protected override bool ReleaseHandle()
    {
        Native.wickra_terminal_free(handle);
        return true;
    }
}
