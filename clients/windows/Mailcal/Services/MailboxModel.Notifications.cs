// New-mail notifications: the core's on/off choice, the scan that feeds them and the state that
// keeps it from piling up. Split into its own partial to keep MailboxModel.cs under the 500-line
// limit.
//
// There is no second timer here, and that is the design (docs/background-sync.md). The desktop's
// live runtime, standing IMAP IDLE watches and the poll each account is configured for, already
// delivers; a mailbox-list signal is the moment it committed something. So the client reads the
// cache that signal published rather than starting a network pass of its own, which keeps
// notifications on the cadence the user chose in Settings rather than one this file invented.
//
// The scan is off the UI thread because it blocks on the core's runtime; everything the user then
// sees is decided back on it. A burst of signals collapses into a single follow-up: an account
// sync raises dozens a second, and one scan per signal would queue work the UI thread waits behind.

using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // Idle -> InFlight -> (Pending) -> Idle. Pending is the whole burst behind the running scan,
    // collapsed into one repeat, so the follow-up reads the settled cache once.
    private enum NewMailScan
    {
        Idle,
        InFlight,
        Pending,
    }

    private NewMailScan _newMailScan = NewMailScan.Idle;

    /// <summary>Settings → Notifications as the core decides it, or <c>null</c> before the core is
    /// up. Read from memory, so the UI thread may ask it on every Settings signal.</summary>
    internal NotificationsPage? Notifications => _app?.NotificationsPage();

    /// <summary>Turns new-mail notifications on or off. The core holds the choice before this
    /// returns and signals Settings.</summary>
    internal void SetNewMailNotifications(bool on)
    {
        Log.Info($"settings: set new-mail notifications={on}");
        DispatchPreference(new PreferenceIntent.SetNewMailNotifications(on));
    }

    // Carries a choice kept beside the other preference files into the core. Called before the
    // first scan can run, so a person who had turned notifications off is not notified by it.
    private static void ImportNotificationsChoice(MailcalApp app)
    {
        var path = NotificationsImport.FileIn(AppPaths.PrefsDir);
        if (NotificationsImport.Run(path, intent => app.Dispatch(new Intent.Preferences(intent))))
        {
            Log.Info("settings: new-mail notifications carried over as off");
        }
    }

    /// <summary>
    /// Reports whatever inbound Inbox mail the live runtime has just committed, and raises a
    /// notification per message the core returns.
    /// </summary>
    /// <remarks>
    /// The core withholds mail that arrived before this session started, so the catch-up sync a
    /// launch begins with is marked seen rather than announced, and seeds a brand-new account
    /// instead of notifying its whole existing inbox. While notifications are off it returns no
    /// mail and still advances its marks, so turning them back on never floods.
    /// </remarks>
    private void CollectNewMail()
    {
        if (_app is not { } app || Accounts.Count == 0)
        {
            return;
        }
        if (_newMailScan != NewMailScan.Idle)
        {
            _newMailScan = NewMailScan.Pending;
            return;
        }
        _newMailScan = NewMailScan.InFlight;
        _ = Task.Run(() =>
        {
            BackgroundSyncOutcome? outcome = null;
            try
            {
                outcome = app.CollectCachedNewMail();
            }
            catch (Exception ex)
            {
                Log.Warn($"new-mail scan failed: {CoreError.Describe(ex)}");
            }
            _ui.TryEnqueue(() => FinishNewMailScan(outcome));
        });
    }

    // Back on the UI thread: decide what the pass says, post it, and run the one collapsed
    // follow-up if the cache moved again while the scan was out. The copy is resolved here rather
    // than in the worker because it comes from the resource context the UI language is set on.
    //
    // Posting is a COM call and so pumps this thread, which can run a queued signal, and with it
    // another CollectNewMail, before this method finishes. That is why the state is still
    // InFlight while posting: the re-entrant call records a Pending and returns, and the line
    // below picks it up, rather than starting a second scan over the first.
    private void FinishNewMailScan(BackgroundSyncOutcome? outcome)
    {
        if (outcome is not null)
        {
            NewMailNotifier.Post(NewMailNotices.For(
                outcome, L10n.NotificationUnknownSender(), L10n.NotificationMoreMessages));
        }
        var repeat = _newMailScan == NewMailScan.Pending;
        _newMailScan = NewMailScan.Idle;
        if (repeat)
        {
            CollectNewMail();
        }
    }
}
