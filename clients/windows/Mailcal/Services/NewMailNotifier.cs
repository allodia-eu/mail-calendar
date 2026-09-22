// New-mail toasts, through the Windows App SDK's AppNotificationManager (docs/background-sync.md).
// The Windows counterpart of the Linux client's desktop-portal adapter, Android's
// NotificationCompat channel and iOS's UNUserNotificationCenter.
//
// Thin on purpose: what a pass SAYS is NewMailNotices, which is WinUI-free and unit-tested. What
// is left here is the three things only the running desktop can answer, whether this process may
// raise a toast at all, putting one on screen, and reading back the message a click named.
//
// The message travels in the toast's own arguments, which is the Windows shape of what macOS puts
// in `userInfo` and Android in its intent extras (docs/background-sync.md). It has to: a click can
// start the app, and then nothing this process remembers is left to look the message up in.
//
// Registration is needed in BOTH build shapes and takes a different call in each. A packaged
// build registers against its MSIX identity, which is where the shell reads the name and icon it
// heads a notification with. An unpackaged one has no manifest to read them from, so it must hand
// them over: registered through the parameterless overload, this app's mail is filed under
// "Mailcal", the executable's own file name.
//
// Skipping registration altogether does not fail either: Show() simply puts nothing on screen,
// which is indistinguishable from a mailbox with no new mail.
//
// ⚠️ REGISTRATION MUST PRECEDE AppInstance.GetCurrent().GetActivatedEventArgs(), which is why Arm()
// is called from Program.Main rather than from the window. A launch that IS a notification click
// has to build an AppNotificationActivatedEventArgs, and the runtime cannot do that without the
// notification platform behind it: unregistered, it does not throw, it FAILS FAST (0xc0000409,
// inside Microsoft.WindowsAppRuntime.dll) before Log.Init has given anything somewhere to write.
// The app then simply vanishes on the one launch the feature exists for, leaving an empty log and
// a WER entry, while every other launch is fine. So Arm() is split from Route(): what a click DOES
// needs the window, which Main has not built yet; what the runtime needs is only that the platform
// is up.

using System.Collections.Generic;
using System.IO;
using System.Threading;
using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;

namespace Allodia.Mailcal.Services;

/// <summary>Raises the local new-mail notifications for a background-sync pass.</summary>
internal static class NewMailNotifier
{
    private static bool _armed;

    // What a click does, once there is a window to do it to. Written on the UI thread and read on
    // the click's own background thread, so it goes through Volatile rather than a plain field.
    private static Action<NotificationTarget?>? _onInvoked;

    // What Arm() found, held until there is a log to say it in. Arm() runs before Log.Init, which
    // drops everything written before it has a path, so saying it there would say it nowhere. The
    // severity travels with it: a registration that failed is a warning, and flattening it to an
    // Info line would bury the one message that explains a desktop which never notifies.
    private static string? _registrationNote;
    private static bool _registrationFailed;

    /// <summary>
    /// Registers this process to raise, and to be activated by, notifications. Call once from
    /// <c>Program.Main</c>, before the activation arguments are read.
    /// </summary>
    public static void Arm()
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
            manager.NotificationInvoked += (_, args) => Invoked(NotificationTarget.From(args.Arguments));
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
            _registrationNote = $"notifications: registered, system setting {manager.Setting}";
        }
        catch (Exception ex)
        {
            // A desktop that will not take our registration costs notifications, nothing else:
            // the scan behind them still runs and still advances the core's marks.
            _registrationNote = $"notifications: could not register ({Describe(ex)})";
            _registrationFailed = true;
        }
    }

    /// <summary>Writes what <see cref="Arm"/> found. Call once the log has a path.</summary>
    public static void LogRegistration()
    {
        if (_registrationNote is not { } note)
        {
            return;
        }
        _registrationNote = null;
        if (_registrationFailed)
        {
            Log.Warn(note);
            return;
        }
        Log.Info(note);
    }

    /// <summary>Installs what a click does, now that there is a window to do it to.</summary>
    /// <param name="onInvoked">
    /// Runs when the user clicks a toast, with the message it named or <c>null</c> where it named
    /// none. It arrives on a background thread, so the handler marshals its own work onto the UI
    /// thread.
    /// </param>
    public static void Route(Action<NotificationTarget?> onInvoked) =>
        Volatile.Write(ref _onInvoked, onInvoked);

    // A click: to the window where there is one, and otherwise to the inbox the window drains as
    // it comes up. The second is the narrow gap between Main registering and the shell being
    // reachable, which a click on a PREVIOUS run's toast lands in; the same place a cold start
    // parks its own, drained by the same line.
    private static void Invoked(NotificationTarget? target)
    {
        if (Volatile.Read(ref _onInvoked) is { } handler)
        {
            handler(target);
            return;
        }
        NotificationOpenInbox.Pending = target;
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
                // What a click opens. Left off the summary, which names no message: a toast with
                // no arguments of ours activates the app and nothing more, which is exactly what
                // a summary should do.
                if (!string.IsNullOrEmpty(notice.MessageKey))
                {
                    var (account, message) =
                        new NotificationTarget(notice.Group, notice.MessageKey).Arguments;
                    builder.AddArgument(NotificationTarget.AccountArgument, account)
                        .AddArgument(NotificationTarget.MessageArgument, message);
                }
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
