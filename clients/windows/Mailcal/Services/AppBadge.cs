// The unread count on the taskbar button (docs/app-badge.md), through the Windows App SDK's
// BadgeNotificationManager. One number, the unified Inbox unread the pane's All Accounts › Inbox
// row shows, and nothing at zero.
//
// The badge outlives the process, and nothing updates it while the app is closed, so the window's
// close clears it rather than leaving a count to go stale. It needs the notification registration
// NewMailNotifier.Arm makes in both build shapes; where that failed, so does this, and it costs
// the badge alone.

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
            // Once per process: where the registration failed, every snapshot fails the same way.
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
