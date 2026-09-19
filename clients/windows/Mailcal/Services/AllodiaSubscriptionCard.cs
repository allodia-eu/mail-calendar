// What the subscription card has to draw, and the reading of a core answer that decides it.
//
// The rules are `purchasing.md`, the contract that ships beside the Allodia Licence, and the core
// holds every one of them. What is here is the few things the card can be in, plus the decisions
// that follow from one answer. Its twins are AllodiaSubscriptionModel.kt on Android and
// MailcalModel.AllodiaSubscription.swift on Apple: keep the states and the wording in step.
//
// ⚠️ **None of this gates a capability.** `AllodiaEntitlement` does, locally and without a network
// call. This is what the card *says*, so a service that cannot be reached costs somebody a
// sentence, never access they have paid for.
//
// WinUI-free AND L10n-free on purpose, the same seam SearchHorizonLine and AttendeeSummary use:
// every word arrives as a parameter or is a company's own name, so Mailcal.Tests can link this
// file and fail on the rules without a Windows host. Which matters here more than most: the
// sentence somebody reads about being charged twice, and whether a cancel button exists at all,
// are both invisible to a screenshot and expensive to get wrong.

using System.Collections.Generic;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>What the subscription card has to draw.</summary>
internal enum AllodiaSubscriptionStatus
{
    /// <summary>
    /// The read is in flight. Distinct from <see cref="Unavailable"/>: nothing has been learned
    /// yet, and a spinner and a failure are different things to put in front of somebody.
    /// </summary>
    Checking,

    /// <summary>The read did not come back, or this deployment has no billing configured.</summary>
    Unavailable,

    /// <summary>
    /// The read could not be made at all, because this device's sign-in predates the permission it
    /// needs. An offer rather than an error: they are signed in, one thing is asleep, and the
    /// ordinary sign-in asks for the full current scope set. Kept apart from
    /// <see cref="Unavailable"/> because the remedies differ: waiting fixes an outage and never
    /// fixes this.
    /// </summary>
    NeedsReauth,

    /// <summary>The service answered.</summary>
    Loaded,
}

/// <summary>
/// The card's whole state: which of the four above, and the answer when it is
/// <see cref="AllodiaSubscriptionStatus.Loaded"/>.
/// </summary>
/// <remarks>
/// There is no "purchase outstanding" half here, unlike Apple's and Android's. Windows ships no
/// store purchase at all (`purchasing.md`, per-platform status), so nothing is ever left on this
/// device to attach, and a card drawing "a purchase has not gone through yet" would be describing
/// a thing that cannot happen here.
/// </remarks>
internal sealed record AllodiaSubscriptionCardState(
    AllodiaSubscriptionStatus Status,
    AllodiaSubscription? Subscription = null,
    /// <summary>
    /// The period whose checkout is being opened, so that button alone shows the spinner, or
    /// <c>null</c>.
    /// </summary>
    AllodiaPlan? Buying = null,
    /// <summary>The last thing worth saying about an attempt, already localised, or <c>null</c>.</summary>
    string? Note = null);

/// <summary>The decisions the card makes from one read, each pure so each is a test.</summary>
internal static class AllodiaSubscriptionCard
{
    /// <summary>
    /// What a read that did not come back is allowed to put on screen.
    /// </summary>
    /// <remarks>
    /// ⚠️ <b>The core's typed answer decides, never the failure's text.</b> A sign-in that predates
    /// the permission this read needs is an offer with a remedy, and drawing it as an outage tells
    /// somebody to wait for something that will never arrive. The account-list card next door
    /// reads the same answer for the same reason.
    /// </remarks>
    internal static AllodiaSubscriptionStatus ReadFailure(AllodiaGrantHealth health) =>
        health == AllodiaGrantHealth.NeedsReauth
            ? AllodiaSubscriptionStatus.NeedsReauth
            : AllodiaSubscriptionStatus.Unavailable;

    /// <summary>
    /// What a biller is called on screen.
    /// </summary>
    /// <remarks>
    /// <b>Allodia, never the payment processor behind it.</b> Naming a processor somebody has never
    /// heard of, inside a message about being charged twice, is how a correct warning reads as a
    /// scam. The bare company name is right here, where the sentence is about who is taking the
    /// money rather than about the app.
    /// </remarks>
    internal static string BillerName(AllodiaBiller biller) => biller switch
    {
        AllodiaBiller.Apple => "Apple",
        AllodiaBiller.Google => "Google Play",
        // A biller this build cannot name still has to appear, or a "you are being charged twice"
        // warning would name only one of the two.
        AllodiaBiller.Unknown unknown => unknown.Label,
        _ => "Allodia",
    };

    /// <summary>
    /// Everyone charging for this account right now, in a stable order: Allodia's own billing
    /// first, then each store in the order the service listed them.
    /// </summary>
    /// <remarks>
    /// ⚠️ <b>"Right now" is the whole of it, and reading every store the service listed was a bug
    /// on the other clients.</b> The list keeps a subscription after it ends, so an account that
    /// bought at one store and later at another carries both, and naming the first named the dead
    /// one.
    /// <para>
    /// Whether a store still has something to manage is a different question, answered by
    /// <see cref="StoreCanBeManaged"/>, and the two part company on the states that matter most.
    /// </para>
    /// </remarks>
    internal static IReadOnlyList<AllodiaBiller> Billers(AllodiaSubscription subscription)
    {
        var billers = new List<AllodiaBiller>();
        if (subscription.Own is { } own && own.Status is not AllodiaOwnStatus.PendingFirstPayment)
        {
            billers.Add(new AllodiaBiller.Allodia());
        }
        // Distinct for the same reason the manage buttons are: two subscriptions at one store is
        // an ordinary shape, and naming that store twice would say somebody is charged twice by it.
        billers.AddRange(subscription.Stores
            .Where(store => StoreIsBilling(store.Status))
            .DistinctBy(store => store.Source)
            .Select(store => store.Source switch
            {
                AllodiaStore.Apple => (AllodiaBiller)new AllodiaBiller.Apple(),
                AllodiaStore.Google => new AllodiaBiller.Google(),
                // The service never reports its own billing as a store, so this arm is unreachable
                // rather than meaningful. Saying "Allodia" is still the right answer if it is
                // reached.
                _ => new AllodiaBiller.Allodia(),
            }));
        return billers;
    }

    /// <summary>
    /// Whether a store subscription is one somebody is still on: renewing, being retried, or
    /// cancelled and running out the period already paid for.
    /// </summary>
    /// <remarks>
    /// The rest grant nothing, and the status is deliberately read arm by arm rather than by
    /// exclusion: a status this build does not know is <b>never read as permission</b>, which is
    /// the rule the core states about the same enum.
    /// </remarks>
    internal static bool StoreIsBilling(AllodiaStoreStatus status) => status
        is AllodiaStoreStatus.Active or AllodiaStoreStatus.Grace or AllodiaStoreStatus.Cancelled;

    /// <summary>
    /// The stores worth offering a way into, one entry per store rather than per subscription.
    /// </summary>
    /// <remarks>
    /// ⚠️ <b>An account can carry more than one subscription at the same store</b>, and it does the
    /// moment somebody resubscribes: the lapsed one is still inside the period it was paid for, so
    /// both are manageable and both were drawn, as two identical buttons opening the same page. A
    /// store's subscription page is the store's, not the subscription's, so one button is the
    /// whole of what there is to offer.
    /// </remarks>
    internal static IReadOnlyList<AllodiaStoreSubscription> ManageableStores(
        AllodiaSubscription subscription) =>
        subscription.Stores
            .Where(store => StoreCanBeManaged(store.Status))
            .DistinctBy(store => store.Source)
            .ToList();

    /// <summary>
    /// Whether the store still has something for this person to do about this subscription.
    /// </summary>
    /// <remarks>
    /// ⚠️ <b>Not the same question as who is billing them, and the two answers differ on exactly
    /// the states somebody needs most.</b> On hold and paused grant nothing, so neither may claim
    /// the "billed by" line, and both are fixed only at the store: a card that failed is replaced
    /// there and a pause is lifted there. Dropping their button strands the person it matters to.
    /// <para>
    /// Expired and revoked are the other way about. Nothing is left to manage, and offering the
    /// route anyway walks somebody into the store's own resubscribe button while another source is
    /// already charging them, which is the duplicate billing this contract warns about rather than
    /// causes.
    /// </para>
    /// </remarks>
    internal static bool StoreCanBeManaged(AllodiaStoreStatus status) => status
        is AllodiaStoreStatus.Active
        or AllodiaStoreStatus.Grace
        or AllodiaStoreStatus.Cancelled
        or AllodiaStoreStatus.OnHold
        or AllodiaStoreStatus.Paused;

    /// <summary>
    /// Whether anything will charge again, which decides between "renews on" and "runs until".
    /// </summary>
    /// <remarks>
    /// A store subscription says so itself; Allodia's own says so by still having a next payment.
    /// Deliberately not a reading of <c>Entitled</c>, which answers a different question.
    /// </remarks>
    internal static bool WillRenew(AllodiaSubscription subscription)
    {
        if (subscription.Stores.Any(store => store.AutoRenewing))
        {
            return true;
        }
        return subscription.Own is { } own
            && own.Status is AllodiaOwnStatus.Active
            && !string.IsNullOrEmpty(own.NextPaymentDate);
    }

    /// <summary>
    /// Whether a charge has failed and is being retried. <b>Access continues</b>, so this is never
    /// drawn as a lapse.
    /// </summary>
    internal static bool InGrace(AllodiaSubscription subscription) =>
        subscription.Stores.Any(store => store.Status is AllodiaStoreStatus.Grace)
        || subscription.Own?.Status is AllodiaOwnStatus.PastDue;

    /// <summary>
    /// The period a switch would move to, which is simply the other one, or <c>null</c> when there
    /// is nothing to switch: no subscription of Allodia's own, or a service that has said this
    /// account may not switch.
    /// </summary>
    /// <remarks>
    /// <c>Actions</c> decides, and nothing else. The service computed it with every biller in
    /// view, which no client is in a position to do: somebody the App Store is charging has a
    /// period, and switching it through this API is exactly what the service refuses.
    /// </remarks>
    internal static AllodiaPlan? SwitchTarget(AllodiaSubscription subscription)
    {
        if (!subscription.Actions.CanSwitchInterval)
        {
            return null;
        }
        return subscription.Own?.Interval switch
        {
            AllodiaPlan.Monthly => AllodiaPlan.Yearly,
            AllodiaPlan.Yearly => AllodiaPlan.Monthly,
            _ => null,
        };
    }

    /// <summary>
    /// Whether to offer starting the subscription again, which is a different question from
    /// whether it has been cancelled: the service decides, because a cancelled subscription whose
    /// period has run out is a new checkout rather than a restart, and only it knows which.
    /// </summary>
    internal static bool OffersRestart(AllodiaSubscription subscription) =>
        subscription.Actions.CanResubscribe
        && subscription.Own?.Status is AllodiaOwnStatus.Cancelled;

    /// <summary>
    /// The day both confirmations talk about: what has already been paid for, as the service sent
    /// it, or the empty string when it sent none.
    /// </summary>
    /// <remarks>
    /// ⚠️ The date is the whole reassurance. Somebody cancelling wants to know they are not losing
    /// what they have already paid for, and a cancellation with no date reads as "it stops now",
    /// which is the one thing it does not do.
    /// </remarks>
    internal static string WriteDate(AllodiaSubscription subscription) =>
        subscription.Own?.CurrentPeriodEnd ?? subscription.CurrentPeriodEnd ?? string.Empty;
}
