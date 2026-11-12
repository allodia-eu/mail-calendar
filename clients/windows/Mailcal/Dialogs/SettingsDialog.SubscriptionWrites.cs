// The three things somebody may do to the subscription Allodia bills directly, as the core offers
// them, and the two that are asked first.
//
// Both confirmations are drawn inline (ConfirmPanel): Settings is itself a ContentDialog, and WinUI
// allows one at a time per XamlRoot.

using System;
using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // Which confirmation is open, if any. Both change what somebody is charged and neither says so
    // anywhere else: a cancellation is silent until the period runs out, and a switch until a date
    // that may be a month away. So both are asked before they are done.
    private enum PendingSubscriptionWrite
    {
        None,
        Cancel,
        Switch,
    }

    private PendingSubscriptionWrite _pendingWrite = PendingSubscriptionWrite.None;

    /// <summary>What may be done to Allodia's own subscription, each through its control.</summary>
    private void Writes(StackPanel panel, AllodiaSubscriptionView view)
    {
        // A question about a write the core no longer offers is not asked any more.
        if ((_pendingWrite == PendingSubscriptionWrite.Switch && view.Switch is null)
            || (_pendingWrite == PendingSubscriptionWrite.Cancel && view.Cancel is null))
        {
            _pendingWrite = PendingSubscriptionWrite.None;
        }
        // ⚠️ The date is the whole reassurance: somebody cancelling wants to know they are not losing
        // what they have already paid for, and a cancellation with no date reads as "it stops now".
        var day = AllodiaSubscriptionFormat.Date(view.PaidThrough, Culture) ?? string.Empty;
        if (view.Switch is { } change)
        {
            var to = change.To;
            Asking(
                panel,
                PendingSubscriptionWrite.Switch,
                to == AllodiaPlan.Yearly
                    ? L10n.SettingsSubscriptionSwitchYearly()
                    : L10n.SettingsSubscriptionSwitchMonthly(),
                change.Control,
                "SubscriptionSwitch",
                // ⚠️ **No price here, deliberately.** A price change never reaches somebody who
                // already subscribed, so today's list price is not what this subscriber will be
                // charged. The amount comes back from the switch itself and is said afterwards.
                new ConfirmPanel(
                    L10n.SettingsSubscriptionSwitchTitle(),
                    L10n.SettingsSubscriptionSwitchBody(day),
                    L10n.ActionUpdate(),
                    "SubscriptionSwitchAsk",
                    Answered(() => _model.SwitchAllodiaInterval(to))));
        }
        // No question: starting again is what somebody came here to do.
        if (view.Resubscribe is { } restart)
        {
            panel.Children.Add(ControlComponents.Button(
                L10n.SettingsSubscriptionResubscribe(), restart, "SubscriptionResubscribe", _model.ResubscribeToAllodia));
        }
        if (view.Cancel is { } cancel)
        {
            Asking(
                panel,
                PendingSubscriptionWrite.Cancel,
                L10n.SettingsSubscriptionCancel(),
                cancel,
                "SubscriptionCancel",
                new ConfirmPanel(
                    L10n.SettingsSubscriptionCancelTitle(),
                    L10n.SettingsSubscriptionCancelBody(day),
                    L10n.SettingsSubscriptionCancel(),
                    "SubscriptionCancelAsk",
                    Answered(_model.CancelAllodiaSubscription),
                    // **Never "Cancel" for the way out.** On this question that word is the thing
                    // being asked about, so the answer that does nothing says what it keeps.
                    dismiss: L10n.SettingsSubscriptionCancelKeep()));
        }
    }

    // A button that opens its question, and the question beneath it, still open after a redraw
    // while the core offers the write. Its go-ahead is held as the button is.
    private void Asking(
        StackPanel panel,
        PendingSubscriptionWrite which,
        string label,
        ControlState control,
        string automationId,
        ConfirmPanel confirm)
    {
        panel.Children.Add(ControlComponents.Button(label, control, automationId, () =>
        {
            _pendingWrite = which;
            confirm.Show();
        }));
        confirm.Enabled = control.Enabled;
        if (_pendingWrite == which)
        {
            confirm.Show();
        }
        panel.Children.Add(confirm.Element);
    }

    // What a question's answer does: closes it, and on yes sends the write.
    private Action<bool> Answered(Action write) => yes =>
    {
        _pendingWrite = PendingSubscriptionWrite.None;
        if (yes)
        {
            write();
        }
    };

    // The catalog's words for what a write came to.
    private static AllodiaSubscriptionWords SubscriptionWords() => new(
        L10n.SettingsSubscriptionCancelled,
        L10n.SettingsSubscriptionResubscribed(),
        L10n.SettingsSubscriptionSwitched,
        L10n.SettingsSubscriptionWriteFailed(),
        L10n.SettingsSubscriptionRefusedAlreadyCancelled(),
        L10n.SettingsSubscriptionRefusedAlreadyActive(),
        L10n.SettingsSubscriptionRefusedAlreadyOnInterval(),
        L10n.SettingsSubscriptionRefusedNotSwitchable(),
        L10n.SettingsSubscriptionRefusedNotFound());
}
