// Settings → Allodia account → Subscription: what is being charged, by whom, until when, the way
// to start one, and the three things somebody may do to a subscription Allodia bills directly.
// Its twins are AllodiaSubscriptionCard.kt on Android and AllodiaSubscriptionSettings.swift on
// Apple; keep the wording in step.
//
// The rules are `purchasing.md`, the contract beside the Allodia Licence, and the core holds every
// one of them. **What this file draws is what the core answered**: `Actions` decides which buttons
// exist and nothing here works it out for itself, and a store's subscription is changed only at
// that store, so its button opens the page the service sent.
//
// ⚠️ **A price is drawn, never parsed and never assembled.** Windows sells through Allodia's own
// checkout and no store, so the two prices come from the service as minor units and are formatted
// here (AllodiaSubscriptionFormat), which is the same split a store's formatted string is, arrived
// at from the other direction.
//
// Its own partial, like every other category here, so SettingsDialog.cs stays clear of the
// 500-line limit.

using System.Globalization;
using System.Linq;
using System.Threading.Tasks;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // Which confirmation is on screen, if any. Both of these change what somebody is charged, and
    // neither says so anywhere else: a cancellation is silent until the period runs out, and a
    // switch is silent until a date that may be a month away. So both are asked before they are
    // done.
    private enum PendingSubscriptionWrite
    {
        None,
        Cancel,
        Switch,
    }

    private AllodiaSubscriptionCardState _subscription =
        new(AllodiaSubscriptionStatus.Checking);
    private PendingSubscriptionWrite _pendingWrite = PendingSubscriptionWrite.None;
    // Whose subscription the card has asked about, or null when it has asked about nobody's.
    //
    // The panel is rebuilt on every Apply, so a plain "have I asked yet" flag is what stops every
    // button press firing another network round trip. It is keyed on the ACCOUNT rather than being
    // a bare bool because both of the things that change the answer happen inside one open dialog:
    // signing in again, which is the whole remedy the reauth state offers and would otherwise leave
    // its own prompt on screen for ever; and signing out and in as somebody else, which would
    // otherwise leave one person's subscription drawn under another person's address.
    private string? _subscriptionAskedFor;
    // A write is in flight, so the buttons that would start a second one are disabled.
    private bool _subscriptionBusy;
    // A re-read is already going, so a second return to the window does not start another.
    private bool _subscriptionRefreshing;

    /// <summary>
    /// The subscription card, or <c>null</c> when nobody is signed in: a subscription belongs to
    /// an account, and there is nothing to say about one that does not exist.
    /// </summary>
    private UIElement? BuildAllodiaSubscription()
    {
        if (_model.SignedInAllodiaAccount() is not { } account)
        {
            return null;
        }
        // The read runs when the card first appears rather than when the app connects: it is a
        // network round trip, and a category nobody opened is not worth one. It runs again when the
        // account under it changes, which is what makes signing in again a remedy rather than a
        // button that leaves the same prompt on screen.
        if (_subscriptionAskedFor != account.Email)
        {
            _subscriptionAskedFor = account.Email;
            _subscription = new AllodiaSubscriptionCardState(AllodiaSubscriptionStatus.Checking);
            _ = ReadSubscriptionAsync();
        }
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(Heading(L10n.SettingsSubscriptionHeading()));
        panel.Children.Add(SubscriptionState());
        if (_subscription.Note is { } note)
        {
            panel.Children.Add(Description(note));
        }
        return panel;
    }

    // Leaving the category is what closes this card. The note answers an attempt somebody has just
    // made, so it survives the re-read that follows one, and reading "your next payment becomes X"
    // on opening Settings a week later is a statement about nothing that just happened.
    private void CloseAllodiaSubscription(string tag)
    {
        if (tag == "allodia")
        {
            return;
        }
        _pendingWrite = PendingSubscriptionWrite.None;
        _subscriptionBusy = false;
        ForgetAllodiaSubscriptionRead();
    }

    // Drops what the card knows, so the next build of it asks again.
    private void ForgetAllodiaSubscriptionRead()
    {
        _subscription = new AllodiaSubscriptionCardState(AllodiaSubscriptionStatus.Checking);
        _subscriptionAskedFor = null;
    }

    /// <summary>
    /// The window came back to the front, so the card asks again.
    /// </summary>
    /// <remarks>
    /// ⚠️ <b>A subscription changes in places this app is not, so the one read it made when the
    /// card opened can be stale with nothing here having happened.</b> The case that shows it is
    /// this platform's own checkout, which finishes in a **browser**: the person comes back having
    /// paid and the card still offers to sell them what they just bought. It is the same staleness
    /// after buying on a phone, subscribing on the website, a store cancelling at renewal, or a
    /// charge failing, and none of those involve this app at all, which is why the remedy is the
    /// app being looked at again rather than anything the checkout could hand back.
    /// <para>
    /// Bounded by the card being on screen: <c>_subscriptionAskedFor</c> is set only while the
    /// Allodia category is open with somebody signed in, so an ordinary alt-tab anywhere else in
    /// the app costs nothing. The drawn answer stays up until the new one arrives, rather than
    /// dropping to the spinner, because coming back to a screen that says "Checking…" every time
    /// is worse than briefly reading the answer from a moment ago.
    /// </para>
    /// </remarks>
    internal void OnHostActivated(WindowActivationState state)
    {
        if (state == WindowActivationState.Deactivated
            || _subscriptionAskedFor is null
            || _subscriptionRefreshing)
        {
            return;
        }
        _ = RefreshSubscriptionAsync();
    }

    private async Task RefreshSubscriptionAsync()
    {
        _subscriptionRefreshing = true;
        try
        {
            await ReadSubscriptionAsync();
        }
        finally
        {
            _subscriptionRefreshing = false;
        }
    }

    private UIElement SubscriptionState() => _subscription.Status switch
    {
        AllodiaSubscriptionStatus.Checking => Busy(L10n.SettingsSubscriptionChecking()),
        // Deliberately quiet. A read that did not arrive costs a sentence, never access: whether a
        // capability is on is the entitlement's answer, and it is local.
        AllodiaSubscriptionStatus.Unavailable =>
            Description(L10n.SettingsSubscriptionUnavailable()),
        // An offer rather than an error: they are signed in and this one read is asleep, and the
        // ordinary sign-in asks for the full current scope set.
        AllodiaSubscriptionStatus.NeedsReauth => Reauth(),
        _ => Answered(_subscription.Subscription!),
    };

    private UIElement Reauth()
    {
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(Description(L10n.SettingsSubscriptionReauth()));
        var again = new Button { Content = L10n.SettingsAllodiaReauthAction() };
        again.Click += (_, _) => StartAllodiaSignIn(create: false);
        panel.Children.Add(again);
        return panel;
    }

    private UIElement Answered(AllodiaSubscription subscription)
    {
        var panel = new StackPanel { Spacing = 6 };
        if (subscription.Entitled)
        {
            Active(panel, subscription);
            Writes(panel, subscription);
        }
        else
        {
            panel.Children.Add(Description(L10n.SettingsSubscriptionFree()));
            Offers(panel, subscription);
        }
        return panel;
    }

    /// <summary>What a paid account says: who is charging, until when, and where to change it.</summary>
    private void Active(StackPanel panel, AllodiaSubscription subscription)
    {
        var billers = AllodiaSubscriptionCard.Billers(subscription);
        var first = billers.Count > 0 ? AllodiaSubscriptionCard.BillerName(billers[0]) : null;
        if (first is not null)
        {
            panel.Children.Add(new TextBlock
            {
                Text = L10n.SettingsSubscriptionBilledBy(first),
                TextWrapping = TextWrapping.Wrap,
            });
        }
        if (AllodiaSubscriptionFormat.Date(subscription.CurrentPeriodEnd, Culture) is { } end)
        {
            panel.Children.Add(Description(
                AllodiaSubscriptionCard.WillRenew(subscription)
                    ? L10n.SettingsSubscriptionRenews(end)
                    : L10n.SettingsSubscriptionEnds(end)));
        }
        // Not a lapse, and never drawn as one: the biller is retrying and access continues.
        if (AllodiaSubscriptionCard.InGrace(subscription) && first is not null)
        {
            panel.Children.Add(Description(L10n.SettingsSubscriptionGrace(first)));
        }
        // Reported, never resolved. Cancelling one of them without asking is a decision about
        // somebody else's money, so each exit is offered and none is taken.
        if (subscription.DuplicateBilling.Length > 0)
        {
            var warning = Description(L10n.SettingsSubscriptionDuplicate(string.Join(
                ", ",
                subscription.DuplicateBilling.Select(AllodiaSubscriptionCard.BillerName))));
            warning.Opacity = 1;
            warning.Foreground = _brushes.Of(ThemePalette.Critical);
            panel.Children.Add(warning);
        }
        // A store's subscription is the store's to change, so this opens its page rather than
        // offering a cancel button that would have nothing to call. Drawn for every store that
        // still has something to do rather than for a recognised one.
        foreach (var store in AllodiaSubscriptionCard.ManageableStores(subscription))
        {
            var biller = store.Source == AllodiaStore.Google ? "Google Play" : "Apple";
            var manage = new Button { Content = L10n.SettingsSubscriptionManage(biller) };
            var taken = store;
            manage.Click += (_, _) => _model.OpenAllodiaStorePage(taken);
            panel.Children.Add(manage);
        }
    }

    /// <summary>The two periods, at Allodia's own prices, which is the only shop this build has.</summary>
    private void Offers(StackPanel panel, AllodiaSubscription subscription)
    {
        // `CanStartCheckout` is false for anybody already paying through any source, which is what
        // stops this build selling a second subscription to somebody a store is already charging.
        if (!subscription.CheckoutAvailable || !subscription.Actions.CanStartCheckout)
        {
            return;
        }
        // In the core's order, which is what puts yearly first: the cheaper rate per month, decided
        // once for every client rather than by each of them sorting its own way.
        foreach (var offer in _model.AllodiaOffers(subscription.Prices, Culture))
        {
            // **The period is on the button, not under it.** It is the thing being chosen, so a
            // row of buttons that all read "Subscribe" makes the reader pair each one with a line
            // of small print to find out what it does.
            var buy = new Button
            {
                Content = offer.Plan == AllodiaPlan.Yearly
                    ? L10n.SettingsSubscriptionBuyYearly(offer.DisplayPrice)
                    : L10n.SettingsSubscriptionBuyMonthly(offer.DisplayPrice),
                IsEnabled = _subscription.Buying is null,
            };
            var taken = offer.Plan;
            buy.Click += (_, _) => _ = BuySubscriptionAsync(taken);
            panel.Children.Add(buy);
            // What renewing means, beside the button and not a click away: somebody agreeing to a
            // recurring charge is owed it.
            panel.Children.Add(Description(L10n.SettingsSubscriptionTerms()));
        }
    }

    private static UIElement Busy(string text)
    {
        var busy = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        busy.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
        busy.Children.Add(new TextBlock { Text = text, Opacity = 0.7 });
        return busy;
    }

    // The culture the app's Language choice pinned, which is what every other date here formats
    // against (AppCulture).
    private static CultureInfo Culture => CultureInfo.CurrentCulture;

    private async Task ReadSubscriptionAsync()
    {
        var read = await _model.ReadAllodiaSubscriptionAsync();
        // The note outlives the read that follows a write: it is the only place that write's
        // answer is said, and the read is what proves it happened.
        _subscription = read with { Note = _subscription.Note };
        Apply(() => { });
    }
}
