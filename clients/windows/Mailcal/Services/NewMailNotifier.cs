// New-mail toasts, through the Windows App SDK's AppNotificationManager (docs/background-sync.md).
// The Windows counterpart of the Linux client's desktop-portal adapter, Android's
// NotificationCompat channel and iOS's UNUserNotificationCenter.
//
// Thin on purpose: what a pass SAYS is NewMailNotices, which is WinUI-free and unit-tested. What
// is left here is the two things only the running desktop can answer, whether this process may
// raise a toast at all, and putting one on screen.
//
// Registration is needed in BOTH build shapes and takes a different call in each. A packaged
// build registers against its MSIX identity, which is where the shell reads the name and icon it
// heads a notification with. An unpackaged one has no manifest to read them from, so it must hand
// them over: registered through the parameterless overload, this app's mail is filed under
// "Mailcal", the executable's own file name.
//
// Skipping registration altogether does not fail either: Show() simply puts nothing on screen,
// which is indistinguishable from a mailbox with no new mail.

using System.IO;
using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;

namespace Allodia.Mailcal.Services;

/// <summary>Raises the local new-mail notifications for a background-sync pass.</summary>
internal static class NewMailNotifier
{
    private static bool _armed;

    /// <summary>
    /// Registers this process to raise (and be activated by) notifications. Call once at launch,
    /// before the core can report any mail.
    /// </summary>
    /// <param name="onInvoked">
    /// Runs when the user clicks a toast. It arrives on a background thread, so the handler
    /// marshals its own work onto the UI thread.
    /// </param>
    public static void Arm(Action onInvoked)
    {
        if (_armed)
        {
            return;
        }
        try
        {
            var manager = AppNotificationManager.Default;
            // Subscribed before Register, so a click on a toast raised by a previous run of this
            // app cannot arrive at a handler that is not there yet.
            manager.NotificationInvoked += (_, _) => onInvoked();
            if (AppIdentity.IsPackaged)
            {
                manager.Register();
            }
            else
            {
                manager.Register(L10n.AppTitle(), AppIconUri());
            }
            _armed = true;
            // With what Windows itself says about them. A user (or a policy) can switch this app's
            // notifications off in the system's own settings, and then everything below succeeds
            // and nothing appears; without this line a support log cannot tell that from a mailbox
            // that received nothing. A closed enum, so it carries no content.
            Log.Info($"notifications: registered, system setting {manager.Setting}");
        }
        catch (Exception ex)
        {
            // A desktop that will not take our registration costs notifications, nothing else:
            // the scan behind them still runs and still advances the core's marks.
            Log.Warn($"notifications: could not register ({Describe(ex)})");
        }
    }

    // The brand icon laid down beside the exe, which the shell draws the notification with. A
    // path that is not there still registers; the notification is then drawn with the generic
    // application icon rather than refused.
    private static Uri AppIconUri() =>
        new(Path.Combine(AppContext.BaseDirectory, "Images", "app.ico"));

    // The failure, as a support log may carry it: the exception type and its HRESULT, which is
    // what tells a refused registration from a missing runtime component. Never the message,
    // which a COM layer may build from the payload, and a payload here is sender and subject.
    private static string Describe(Exception ex) =>
        $"{ex.GetType().Name} 0x{ex.HResult:X8}";

    /// <summary>Releases the registration as the app closes. Safe to call unarmed.</summary>
    public static void Disarm()
    {
        if (!_armed)
        {
            return;
        }
        _armed = false;
        try
        {
            AppNotificationManager.Default.Unregister();
        }
        catch (Exception ex)
        {
            Log.Warn($"notifications: could not unregister ({Describe(ex)})");
        }
    }

    /// <summary>
    /// Puts one toast on screen per notice. Tag and group come from the notice, so a message
    /// reported twice replaces its own notification rather than stacking a second one.
    /// </summary>
    public static void Post(IReadOnlyList<NewMailNotice> notices)
    {
        if (!_armed || notices.Count == 0)
        {
            return;
        }
        var manager = AppNotificationManager.Default;
        foreach (var notice in notices)
        {
            try
            {
                var builder = new AppNotificationBuilder()
                    .AddText(notice.Title)
                    .AddText(notice.Body);
                // The toast template's three text elements are exactly sender, subject and
                // snippet. Added only when there is one: an account whose body sync has not run
                // yet has none, and a third element holding nothing draws as a blank line.
                if (!string.IsNullOrWhiteSpace(notice.Preview))
                {
                    builder.AddText(notice.Preview);
                }
                var built = builder.BuildNotification();
                built.Tag = notice.Tag;
                built.Group = notice.Group;
                manager.Show(built);
            }
            catch (Exception ex)
            {
                // Deliberately without the notice's own text: a diagnostic log never carries mail
                // content (docs/logging.md), and all three lines here are the message itself.
                Log.Warn($"notifications: could not post ({Describe(ex)})");
            }
        }
        // A count, which is what a support log needs to tell "nothing arrived" from "we never
        // put it on screen" apart.
        Log.Info($"notifications: posted {notices.Count}");
    }
}
