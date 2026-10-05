// An Outbox row's menu (docs/sending.md). Its items differ by the message's state, so it is built
// for each row rather than declared once in the template. What a state lists and which intent an
// item sends are OutboxRows', gated by Mailcal.Tests; this half draws the items and passes the
// choice on.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Views;

public sealed partial class MailListView
{
    // Built when the row's message is set, which is also when the list recycles the row for
    // another message, and when a state change replaces the row's message in the reconcile. The
    // menu stays the row's ContextFlyout so the pointer and the menu key reach it as before.
    private void OnQueuedRowDataContextChanged(FrameworkElement sender, DataContextChangedEventArgs args)
    {
        sender.ContextFlyout = args.NewValue is QueuedRowItem row ? QueuedMenu(row) : null;
    }

    // A message in flight lists its items disabled rather than none: an empty popup says nothing
    // about why. An item that takes the message away for good sits apart behind a separator.
    private MenuFlyout QueuedMenu(QueuedRowItem row)
    {
        var flyout = new MenuFlyout();
        foreach (var action in row.Actions)
        {
            if (OutboxRows.IsDestructive(action) && flyout.Items.Count > 0)
            {
                flyout.Items.Add(new MenuFlyoutSeparator());
            }
            var item = new MenuFlyoutItem { Text = QueuedActionText(action), IsEnabled = row.IsActionable };
            item.Click += (_, _) => Model?.ActOnQueued(row, action);
            flyout.Items.Add(item);
        }
        return flyout;
    }

    // Both ways of sending again read the same, because to the user they are the same request;
    // which intent each sends is what tells them apart.
    private static string QueuedActionText(QueuedAction action) => action switch
    {
        QueuedAction.SendNow => L10n.ActionSendNow(),
        QueuedAction.SendAgain or QueuedAction.ConfirmNotSent => L10n.ActionSendAgain(),
        QueuedAction.Edit => L10n.ActionEditQueued(),
        QueuedAction.Cancel => L10n.ActionCancelSend(),
        QueuedAction.Discard => L10n.ActionDiscard(),
        QueuedAction.MarkSent => L10n.ActionMarkSent(),
        _ => string.Empty,
    };
}
