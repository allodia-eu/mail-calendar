// The three things somebody may do to the subscription Allodia bills directly, plus the checkout
// that starts one: the buttons, the two confirmations, and the sentence each ending earns.
//
// ⚠️ **Only Allodia's own subscription, never a store's.** A store's is changed at that store and
// nowhere else, so the card offers its manage page for those and none of this. What decides whether
// a button exists at all is `Actions`, which the service computed with every biller in view; a
// client is in no position to work that out and does not try.
//
// Both confirmations are drawn **inline**, in the panel, rather than as a dialog of their own:
// Settings is itself a ContentDialog, and WinUI allows one at a time per XamlRoot. It is the same
// shape the signature delete already uses next door.

using System;
using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    /// <summary>
    /// What may be done to Allodia's own subscription, and whichever confirmation is open.
    /// </summary>
    private void Writes(StackPanel panel, AllodiaSubscription subscription)
    {
        var switchTo = AllodiaSubscriptionCard.SwitchTarget(subscription);
        if (switchTo is { } target)
        {
            var move = new Button
            {
                Content = target == AllodiaPlan.Yearly
                    ? L10n.SettingsSubscriptionSwitchYearly()
                    : L10n.SettingsSubscriptionSwitchMonthly(),
                IsEnabled = !_subscriptionBusy,
            };
            move.Click += (_, _) => Apply(() => _pendingWrite = PendingSubscriptionWrite.Switch);
            panel.Children.Add(move);
        }
        if (AllodiaSubscriptionCard.OffersRestart(subscription))
        {
            // No confirmation: starting again is what somebody came here to do, and it charges
            // nothing today that the cancellation had not already left running.
            var restart = new Button
            {
                Content = L10n.SettingsSubscriptionResubscribe(),
                IsEnabled = !_subscriptionBusy,
            };
            restart.Click += (_, _) => _ = RunSubscriptionWriteAsync(
                () => _model.ResubscribeToAllodiaAsync());
            panel.Children.Add(restart);
        }
        if (subscription.Actions.CanCancel)
        {
            var cancel = new Button
            {
                Content = L10n.SettingsSubscriptionCancel(),
                IsEnabled = !_subscriptionBusy,
            };
            cancel.Click += (_, _) => Apply(() => _pendingWrite = PendingSubscriptionWrite.Cancel);
            panel.Children.Add(cancel);
        }

        var day = AllodiaSubscriptionFormat.Date(
            AllodiaSubscriptionCard.WriteDate(subscription), Culture) ?? string.Empty;
        switch (_pendingWrite)
        {
            case PendingSubscriptionWrite.Cancel:
                panel.Children.Add(Confirm(
                    L10n.SettingsSubscriptionCancelTitle(),
                    // ⚠️ The date is the whole reassurance: somebody cancelling wants to know they
                    // are not losing what they have already paid for. A cancellation with no date
                    // reads as "it stops now", which is the one thing it does not do.
                    L10n.SettingsSubscriptionCancelBody(day),
                    L10n.SettingsSubscriptionCancel(),
                    // **Never "Cancel" for the way out.** On this confirmation that word is the
                    // thing being asked about, so the button that does nothing has to say what it
                    // keeps.
                    L10n.SettingsSubscriptionCancelKeep(),
                    () => _model.CancelAllodiaSubscriptionAsync()));
                break;
            case PendingSubscriptionWrite.Switch when switchTo is { } moving:
                panel.Children.Add(Confirm(
                    L10n.SettingsSubscriptionSwitchTitle(),
                    // ⚠️ **No price here, deliberately.** What this subscriber is charged is not
                    // today's list price, because a price change never reaches somebody who
                    // already subscribed, and quoting the list price would tell a long-standing
                    // subscriber a number they will not be charged. The amount comes back from the
                    // switch itself and is said afterwards.
                    L10n.SettingsSubscriptionSwitchBody(day),
                    L10n.ActionUpdate(),
                    L10n.ActionCancel(),
                    () => _model.SwitchAllodiaIntervalAsync(moving)));
                break;
            default:
                break;
        }
    }

    // A confirmation in the panel: what is being asked, what it costs, and the two ways out.
    private UIElement Confirm(
        string title,
        string body,
        string confirm,
        string dismiss,
        Func<Task<AllodiaWriteResult>> write)
    {
        var stack = new StackPanel { Spacing = 6 };
        stack.Children.Add(new TextBlock { Text = title, TextWrapping = TextWrapping.Wrap });
        stack.Children.Add(Description(body));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var yes = new Button { Content = confirm };
        yes.Click += (_, _) =>
        {
            _pendingWrite = PendingSubscriptionWrite.None;
            _ = RunSubscriptionWriteAsync(write);
        };
        var no = new Button { Content = dismiss };
        no.Click += (_, _) => Apply(() => _pendingWrite = PendingSubscriptionWrite.None);
        buttons.Children.Add(yes);
        buttons.Children.Add(no);
        stack.Children.Add(buttons);
        return new Border
        {
            Background = _brushes.Of(ThemePalette.LayerFill),
            CornerRadius = new CornerRadius(6),
            Padding = new Thickness(10),
            Child = stack,
        };
    }

    // Opens Allodia's own checkout, which is the whole of buying on this platform.
    //
    // Nothing waits for an answer: the subscription is created against the account by the checkout
    // page itself, so what tells this app it happened is the next read.
    private async Task BuySubscriptionAsync(AllodiaPlan plan)
    {
        _subscription = _subscription with { Buying = plan, Note = null };
        Apply(() => { });
        var failure = await _model.StartAllodiaCheckoutAsync(plan);
        _subscription = _subscription with
        {
            Buying = null,
            Note = failure is null ? null : RefusalText(failure.Value),
        };
        await ReadSubscriptionAsync();
    }

    // One write: its answer into the card's note, then a re-read.
    //
    // **What the write actually did is the next read's answer, never this one's**: the service
    // recomputes every biller, and a card that edited its own copy would disagree with it. The
    // note is cleared first, because the last write's sentence beside this one's outcome reads as
    // one statement about the wrong thing.
    private async Task RunSubscriptionWriteAsync(Func<Task<AllodiaWriteResult>> write)
    {
        if (_subscriptionBusy)
        {
            return;
        }
        _subscriptionBusy = true;
        _subscription = _subscription with { Note = null };
        Apply(() => { });
        var result = await write();
        _subscriptionBusy = false;
        _subscription = _subscription with { Note = WriteNote(result) };
        await ReadSubscriptionAsync();
    }

    // The sentence one ending earns, or null for the one that has nothing to say here because what
    // happens next is a browser window.
    private string? WriteNote(AllodiaWriteResult result) => result.Outcome switch
    {
        AllodiaWriteOutcome.Cancelled => L10n.SettingsSubscriptionCancelled(
            AllodiaSubscriptionFormat.Date(result.EndDate, Culture) ?? result.EndDate ?? string.Empty),
        AllodiaWriteOutcome.Reactivated => L10n.SettingsSubscriptionResubscribed(),
        AllodiaWriteOutcome.Switched => SwitchedNote(result.Change!),
        AllodiaWriteOutcome.Failed =>
            RefusalText(result.Failure ?? AllodiaWriteFailure.Unexplained),
        _ => null,
    };

    // A refusal's own words, never the core's: UniFFI builds an exception's message out of the
    // variant's fields, so what is available there is the literal text `reason=NotSwitchable`, and
    // an unreachable service carries no message at all. The twins are AllodiaSubscriptionModel.kt
    // and allodia_subscription_facts.rs; keep the wording in step.
    private static string RefusalText(AllodiaWriteFailure failure) => failure switch
    {
        AllodiaWriteFailure.AlreadyCancelled => L10n.SettingsSubscriptionRefusedAlreadyCancelled(),
        AllodiaWriteFailure.AlreadyActive => L10n.SettingsSubscriptionRefusedAlreadyActive(),
        AllodiaWriteFailure.AlreadyOnInterval =>
            L10n.SettingsSubscriptionRefusedAlreadyOnInterval(),
        AllodiaWriteFailure.NotSwitchable => L10n.SettingsSubscriptionRefusedNotSwitchable(),
        AllodiaWriteFailure.NotFound => L10n.SettingsSubscriptionRefusedNotFound(),
        _ => L10n.SettingsSubscriptionWriteFailed(),
    };

    // What the next charge becomes, in the service's own figures.
    //
    // ⚠️ The switch answers an amount with no currency beside it, so it is priced from the
    // subscription's own `Prices.Currency`, which is today's list currency rather than the one this
    // subscriber was charged in. The two differ only for somebody whose billing currency has since
    // changed, which the service does not currently do.
    private string SwitchedNote(AllodiaIntervalChange change)
    {
        var date = AllodiaSubscriptionFormat.Date(change.NextPaymentDate, Culture) ?? string.Empty;
        var amount = AllodiaSubscriptionFormat.MinorUnits(
            change.AmountInCents,
            _subscription.Subscription?.Prices.Currency,
            Culture);
        return L10n.SettingsSubscriptionSwitched(date, amount);
    }
}
