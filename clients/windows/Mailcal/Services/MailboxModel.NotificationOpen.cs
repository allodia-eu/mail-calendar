// Opening the message a clicked new-mail notification named (docs/background-sync.md). Split into
// its own partial to keep MailboxModel.cs under the 500-line limit; the scan that raises the
// notifications is MailboxModel.Notifications.cs.
//
// The rule is the one macOS, iOS/iPadOS and Android already follow. The list on screen comes
// first, so a message in view opens where it is without moving the mailbox off the folder the
// person left it on. Only when it is not there does the mailbox move to the message's account,
// and then for exactly ONE snapshot.
//
// That bound is the whole design. A message can be missing for one snapshot because the account
// was only just selected, which is worth waiting for; it can also be missing for good, because it
// was deleted or sits outside the loaded window. Without the bound the second case would drag the
// view back to the mailbox on every snapshot the person afterwards caused, fighting whatever they
// navigated to next.

using System.Linq;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    private NotificationTarget? _pendingNotification;

    // Whether the mailbox has already been pointed at the pending message's account. What bounds
    // the retrying to the one snapshot that move is worth.
    private bool _notificationNavigated;

    /// <summary>Opens the message a clicked notification named, once the list holds it.</summary>
    internal void OpenNotification(NotificationTarget target)
    {
        // The reading pane lives in the mail surface, so a click arriving over the calendar or
        // Contacts would otherwise open the message behind them and read as a click that did
        // nothing. Here rather than in the retry below, which must not take the surface back a
        // snapshot after the person has moved on.
        ShowMail();
        _pendingNotification = target;
        _notificationNavigated = false;
        TryOpenNotification();
    }

    /// <summary>
    /// The one retry the account switch earns, spent whether or not it finds the message.
    /// </summary>
    /// <remarks>
    /// Called from the mailbox-list reload rather than from <c>Rows.CollectionChanged</c>: the
    /// reconcile removes the old account's rows and adds the new account's within one reload, so a
    /// collection event can arrive with the list half filled, and this bound allows exactly one
    /// look.
    /// </remarks>
    internal void RetryNotificationOpen()
    {
        if (_pendingNotification is not null)
        {
            TryOpenNotification();
        }
    }

    private void TryOpenNotification()
    {
        if (_pendingNotification is not { } target)
        {
            return;
        }
        if (OpenFromList(target))
        {
            _pendingNotification = null;
            return;
        }
        if (_notificationNavigated)
        {
            // The retry is spent and the message was not there: deleted, outside the loaded
            // window, or in a store this build is not looking at. The click ends here, and this is
            // the only thing that distinguishes that from one that opened the message, since both
            // leave the app forward with the mailbox on that account.
            _pendingNotification = null;
            Log.Info("notification click: the message it named is not in the list, nothing opened");
            return;
        }
        // Not in the list on screen: point the mailbox at the account that received it, and let
        // the snapshot that produces answer.
        _notificationNavigated = true;
        SelectAccount(target.Account);
    }

    // Opens the named message where the list currently holds it, or reports that it does not.
    //
    // A conversation is searched through rather than opened at its representative: the
    // notification named one message, and a thread's representative is its latest in-scope
    // message, which on a busy thread is not the one that was announced.
    private bool OpenFromList(NotificationTarget target)
    {
        foreach (var row in Rows)
        {
            if (!row.IsThread)
            {
                if (row.Account == target.Account && row.LatestKey == target.MessageKey)
                {
                    OpenMessage(row);
                    return true;
                }
                continue;
            }
            var message = row.Messages.FirstOrDefault(
                m => m.Account == target.Account && m.Key == target.MessageKey);
            if (message is null)
            {
                continue;
            }
            // Disclosed as well as opened, so the reading pane is showing a row the list also
            // shows. ToggleThread would open the thread's representative instead of this message.
            row.IsExpanded = true;
            _expandedThreads.Add(row.Id);
            OpenThreadMessage(message);
            return true;
        }
        return false;
    }
}
