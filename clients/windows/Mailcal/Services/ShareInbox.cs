// The one share waiting to be opened, handed from the activation to the window.
//
// The twin of MailLinkInbox, and for the same reason: a share can activate the app before there is
// a window to show a composer in or an account to send from. It is parked here and drained by
// MainWindow once the core is up (MainWindow.Share.cs).
//
// It holds a decoded `SharePrefill` rather than the activation, because a `ShareOperation`'s
// access to what was shared is revoked when the operation reports completion, which happens as
// soon as it is read. By the time this is drained, the bytes are already staged in app-private
// storage and the paths in here are ones this app can still open.
using System.Threading;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The share a launch carried, until the window is ready to open it.</summary>
internal static class ShareInbox
{
    private static SharePrefill? _pending;

    /// <summary>
    /// The pending share, or <c>null</c>. Set once by the activation, taken once by the window.
    /// </summary>
    /// <remarks>
    /// Volatile for the reason <see cref="MailLinkInbox"/> is: the two touches are on different
    /// threads, and a share lost to a torn read is a set of files the user watched disappear.
    /// </remarks>
    internal static SharePrefill? Pending
    {
        get => Volatile.Read(ref _pending);
        set => Volatile.Write(ref _pending, value);
    }

    /// <summary>Takes the pending share, leaving none behind, so it opens only once.</summary>
    internal static SharePrefill? Take() => Interlocked.Exchange(ref _pending, null);
}
