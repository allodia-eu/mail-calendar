// The one mail link waiting to be opened, handed from process start to the window.
//
// A cold start FROM a `mailto:` link arrives in Main, where there is no window to show a composer
// in and no account list to send from. The link is parked here and drained by MainWindow once the
// core is up (MainWindow.MailLink.cs). A link arriving at an already-running app skips this
// entirely and goes straight to the window.
using System.Threading;

namespace Allodia.Mailcal.Services;

/// <summary>The mail link a cold start carried, until the window is ready to open it.</summary>
internal static class MailLinkInbox
{
    private static string? _pending;

    /// <summary>
    /// The pending link, or <c>null</c>. Set once in <c>Main</c>, taken once by the window.
    /// </summary>
    /// <remarks>
    /// Interlocked because the two touches are on different threads, Main's, and the UI thread the
    /// window is built on, and a link silently lost to a torn read would be a click that did
    /// nothing, with no way for the user to tell why.
    /// </remarks>
    internal static string? Pending
    {
        get => Volatile.Read(ref _pending);
        set => Volatile.Write(ref _pending, value);
    }

    /// <summary>Takes the pending link, leaving none behind, so it can only be opened once.</summary>
    internal static string? Take() => Interlocked.Exchange(ref _pending, null);
}
