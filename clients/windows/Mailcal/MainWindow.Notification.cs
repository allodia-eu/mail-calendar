// Answering a click on a new-mail notification, the shell half of what the OS hands us.
//
// Where the message goes is MailboxModel.NotificationOpen.cs. What is left here is the two things
// only the window knows. First, that this process does not hold foreground rights: the click was
// handled by the shell, so a bare Activate() would be ignored and the message would open behind
// whatever the person was looking at. Second, the unsent draft: the composer lives in the
// reading-pane slot, so opening a message over one throws it away, and a notification arrives
// unprompted at any moment, on the same footing as a mail link (docs/background-sync.md).

using Allodia.Mailcal.Services;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // Guards the await below, exactly as _openingMailLink guards the mail link's: a second click
    // while the discard question is up must not open a second message behind the question the
    // person is still answering.
    private bool _openingNotification;

    /// <summary>
    /// Brings the app forward and opens the message the notification named, if it named one.
    /// </summary>
    /// <param name="target">
    /// The message, or <c>null</c> for the overflow summary, which stands for mail it does not
    /// name: that one opens the app and nothing more, on every platform that has this.
    /// </param>
    internal async void OpenNotification(NotificationTarget? target)
    {
        BringToForeground();
        // The only record that a click arrived at all. Everything this feature does after here is
        // silent when it goes wrong, so without this line a click that opened nothing and a click
        // that never reached the app read identically in a support log. Whether one was named, not
        // which: an account id and a provider key are the message (docs/logging.md).
        Log.Info(target is null
            ? "notification click: named no message, bringing the app forward"
            : "notification click: opening the message it named");
        if (target is not { } message || _openingNotification)
        {
            return;
        }
        _openingNotification = true;
        try
        {
            if (!await ConfirmDiscardDraftAsync())
            {
                Log.Info("notification click declined, the open draft was kept");
                return;
            }
            Model.OpenNotification(message);
        }
        finally
        {
            _openingNotification = false;
        }
    }
}
