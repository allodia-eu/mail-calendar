// The unread count on the taskbar button (docs/app-badge.md), through the Windows App SDK's
// BadgeNotificationManager. One number, the unified Inbox unread the pane's All Accounts › Inbox
// row shows, and nothing at zero.
//
// The badge outlives the process, and nothing updates it while the app is closed, so the window's
// close clears it rather than leaving a count to go stale.
//
// Packaged builds only. Every call on BadgeNotificationManager throws E_ILLEGAL_METHOD_CALL in a
// process without package identity, whatever notification registration it holds, so the
// unpackaged dev loop draws no badge and says so once in the log. Both shipped channels are
// packages (docs/windows-channels.md).

using Microsoft.Windows.BadgeNotifications;

namespace Allodia.Mailcal.Services;

/// <summary>Puts the unified Inbox unread on the taskbar button.</summary>
internal static class AppBadge
{
    // What this process last wrote. Null until the first snapshot, so that one always writes and
    // corrects a badge a previous run left behind.
    private static uint? _shown;

    // Set as the window closes: a snapshot still queued behind the close must not put back the
    // count Clear took off, since nothing would take it off again.
    private static bool _closed;

    private static bool _warned;

    /// <summary>Shows <paramref name="unread"/>, or no badge at zero. Asks the shell only when
    /// the number moved.</summary>
    public static void Show(uint unread)
    {
        if (_closed || _shown == unread)
        {
            return;
        }
        if (!AppIdentity.IsPackaged)
        {
            if (!_warned)
            {
                _warned = true;
                Log.Info("app badge: none, this build runs without package identity");
            }
            return;
        }
        try
        {
            if (unread == 0)
            {
                BadgeNotificationManager.Current.ClearBadge();
            }
            else
            {
                BadgeNotificationManager.Current.SetBadgeAsCount(unread);
            }
            // Only once it went, so a write that failed is tried again by the next snapshot.
            _shown = unread;
        }
        catch (Exception ex)
        {
            // Once per process: a shell that refused one write refuses the next snapshot's too.
            if (!_warned)
            {
                _warned = true;
                Log.Warn($"app badge: could not update ({ex.GetType().Name}: 0x{ex.HResult:X8})");
            }
        }
    }

    /// <summary>Takes the badge off as the app closes.</summary>
    public static void Clear()
    {
        Show(0);
        _closed = true;
    }
}
