// Settings → Allodia account → Subscription: what is being charged, by whom, until when, the way
// to start one, and the three things somebody may do to the subscription Allodia bills directly.
//
// **What this file draws is what the core decided** (`MailcalApp.AllodiaSubscriptionCard`,
// docs/waiting.md): which writes exist, whether one runs, which store pages to offer and what a
// write came to. A store's subscription is changed only at that store, so its button opens the
// store's page.
//
// ⚠️ **A price is drawn, never parsed and never assembled.** Windows sells through Allodia's own
// checkout and no store, so the two prices come from the card as minor units and are formatted
// here (AllodiaSubscriptionFormat), in the order the core's OrderAllodiaOffers gives.

using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // Whether the Allodia category is the one shown, so opening it asks the service again and a
    // redraw of it does not.
    private bool _allodiaShown;

    /// <summary>The section, or <c>null</c> while the core draws none: nobody signed in, or a
    /// sign-in the service refused, which the account card says and carries the remedy for.</summary>
    private UIElement? BuildAllodiaSubscription(AllodiaCard account, AllodiaSubscriptionCard card)
    {
        if (card.State is AllodiaSubscriptionState.Hidden)
        {
            _pendingWrite = PendingSubscriptionWrite.None;
            return null;
        }
        var panel = new StackPanel { Spacing = 6 };
        var heading = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        heading.Children.Add(Heading(L10n.SettingsSubscriptionHeading()));
        // A read beside the answer, after a return from the checkout browser: the answer stays drawn.
        if (card.State is AllodiaSubscriptionState.Loaded { View.Refreshing: true })
        {
            heading.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
        }
        panel.Children.Add(heading);
        switch (card.State)
        {
            case AllodiaSubscriptionState.Checking:
                // Nothing read yet, after a sign-in on this page among others: a read already out
                // answers for this one, and the core drops the second.
                _model.RefreshAllodiaSubscription();
                panel.Children.Add(Busy(L10n.SettingsSubscriptionChecking()));
                break;
            // Deliberately quiet: a read that did not arrive costs a sentence, never access.
            case AllodiaSubscriptionState.Unavailable:
                panel.Children.Add(Description(L10n.SettingsSubscriptionUnavailable()));
                break;
            // An offer rather than an error: they are signed in and this one read is asleep, and
            // the ordinary sign-in asks for the full current scope set.
            case AllodiaSubscriptionState.NeedsReauth:
                panel.Children.Add(Description(L10n.SettingsSubscriptionReauth()));
                panel.Children.Add(ControlComponents.Button(
                    L10n.SettingsAllodiaReauthAction(), account.SignIn, "SubscriptionSignInAgain",
                    () => _model.SignInToAllodia()));
                break;
            case AllodiaSubscriptionState.Loaded { View: var view }:
                LoadedSection(panel, view);
                break;
            default:
                break;
        }
        if (card.Outcome is { } outcome)
        {
            panel.Children.Add(Description(
                AllodiaSubscriptionRules.OutcomeText(outcome, SubscriptionWords(), Culture)));
        }
        return panel;
    }

    // The category `tag` is about to be drawn. Opening Allodia asks the service
    // again; leaving it closes a confirmation it had open.
    private void ShowingAllodiaSubscription(string tag)
    {
        var opening = tag == "allodia" && !_allodiaShown;
        _allodiaShown = tag == "allodia";
        if (!_allodiaShown)
        {
            _pendingWrite = PendingSubscriptionWrite.None;
        }
        else if (opening)
        {
            _model.RefreshAllodiaSubscription();
        }
    }

    /// <summary>
    /// The window came back to the front, which is what returning from the checkout browser looks
    /// like here, so the section asks again.
    /// </summary>
    /// <remarks>
    /// A subscription also changes where this app is not: on a phone, on the website, at a store
    /// renewing or cancelling. Only a section on screen with an answer drawn asks, so an alt-tab
    /// anywhere else costs no round trip; a read still out answers for the return.
    /// </remarks>
    internal void OnHostActivated(WindowActivationState state)
    {
        if (state != WindowActivationState.Deactivated
            && CurrentCategory() == "allodia"
            && _model.AllodiaSubscriptionCard() is { } card
            && AllodiaSubscriptionRules.WorthRereading(card.State))
        {
            _model.RefreshAllodiaSubscription();
        }
    }

    private void LoadedSection(StackPanel panel, AllodiaSubscriptionView view)
    {
        if (view.Entitled)
        {
            Active(panel, view);
            StorePages(panel, view);
            Writes(panel, view);
        }
        else
        {
            panel.Children.Add(Description(L10n.SettingsSubscriptionFree()));
            // On hold and paused grant nothing and are put right only at the store, so a free
            // account offers its page too.
            StorePages(panel, view);
            Offers(panel, view);
        }
    }

    /// <summary>What a paid account says: who is charging and until when.</summary>
    private void Active(StackPanel panel, AllodiaSubscriptionView view)
    {
        var biller = view.BilledBy is { } billedBy ? AllodiaSubscriptionRules.BillerName(billedBy) : null;
        if (biller is not null)
        {
            panel.Children.Add(new TextBlock
            {
                Text = L10n.SettingsSubscriptionBilledBy(biller),
                TextWrapping = TextWrapping.Wrap,
            });
        }
        if (AllodiaSubscriptionFormat.Date(view.PeriodEnd, Culture) is { } end)
        {
            panel.Children.Add(Description(
                view.Renews ? L10n.SettingsSubscriptionRenews(end) : L10n.SettingsSubscriptionEnds(end)));
        }
        // Not a lapse, and never drawn as one: the charge is being retried and access continues.
        if (view.Grace && biller is not null)
        {
            panel.Children.Add(Description(L10n.SettingsSubscriptionGrace(biller)));
        }
        // Reported, never resolved: cancelling one without asking is a decision about somebody's
        // money, so each exit is offered and none is taken.
        if (view.DuplicateBilling.Length > 0)
        {
            panel.Children.Add(CriticalLine(L10n.SettingsSubscriptionDuplicate(string.Join(
                ", ", view.DuplicateBilling.Select(AllodiaSubscriptionRules.BillerName)))));
        }
    }

    /// <summary>One button per store with something left to manage there, each opening that
    /// store's page. It opens a page and starts no work, so the core gives it no state.</summary>
    private static void StorePages(StackPanel panel, AllodiaSubscriptionView view)
    {
        foreach (var page in view.StorePages)
        {
            var manage = new Button
            {
                Content = L10n.SettingsSubscriptionManage(AllodiaSubscriptionRules.StoreName(page.Store)),
            };
            manage.Click += (_, _) => MailboxModel.OpenAllodiaStorePage(page);
            panel.Children.Add(manage);
        }
    }

    /// <summary>The two periods at Allodia's own prices, which is the only shop this build has,
    /// when the core says they may be sold.</summary>
    private void Offers(StackPanel panel, AllodiaSubscriptionView view)
    {
        if (view.Checkout is not { } checkout)
        {
            return;
        }
        foreach (var offer in _model.AllodiaOffers(checkout.Prices, Culture))
        {
            // **The period is on the button, not under it.** It is the thing being chosen.
            var plan = offer.Plan;
            panel.Children.Add(ControlComponents.Button(
                plan == AllodiaPlan.Yearly
                    ? L10n.SettingsSubscriptionBuyYearly(offer.DisplayPrice)
                    : L10n.SettingsSubscriptionBuyMonthly(offer.DisplayPrice),
                AllodiaSubscriptionRules.CheckoutControl(checkout, plan),
                $"SubscriptionBuy{plan}",
                () => _model.StartAllodiaCheckout(plan)));
        }
        // What renewing means, beside the buttons and not a click away: somebody agreeing to a
        // recurring charge is owed it.
        panel.Children.Add(Description(L10n.SettingsSubscriptionTerms()));
    }

    private static StackPanel Busy(string text)
    {
        var busy = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        busy.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
        busy.Children.Add(new TextBlock { Text = text, Opacity = 0.7 });
        return busy;
    }

    // The culture the app's Language choice pinned, which is what every other date here formats
    // against (AppCulture).
    private static CultureInfo Culture => CultureInfo.CurrentCulture;
}
