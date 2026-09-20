// What the subscription card decides from one account-service read (`purchasing.md`).
//
// Every rule here fails silently on screen. A card that names the wrong biller looks like a card;
// a cancel button offered on a subscription the App Store is charging looks like a button, and
// pressing it is a refusal the person cannot act on; and a manage link drawn twice for one store
// says somebody is being charged twice by it, which is the warning this contract reserves for
// when it is true. None of it is reachable from a screenshot or from UI Automation.

using System.Linq;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AllodiaSubscriptionCardTests
{
    [Fact]
    public void ASignInOlderThanThePermissionIsAnOfferAndNotAnOutage()
    {
        // The remedies are opposites: waiting fixes an outage and never fixes this, so a read that
        // did not come back may not decide the sentence on its own.
        Assert.Equal(
            AllodiaSubscriptionStatus.NeedsReauth,
            AllodiaSubscriptionCard.ReadFailure(AllodiaGrantHealth.NeedsReauth));
        Assert.Equal(
            AllodiaSubscriptionStatus.Unavailable,
            AllodiaSubscriptionCard.ReadFailure(AllodiaGrantHealth.Ok));
        Assert.Equal(
            AllodiaSubscriptionStatus.Unavailable,
            AllodiaSubscriptionCard.ReadFailure(AllodiaGrantHealth.SignedOut));
    }

    [Fact]
    public void AllodiaIsNamedAllodiaAndNotThePaymentProcessorBehindIt()
    {
        // Naming a processor somebody has never heard of, inside a message about being charged
        // twice, is how a correct warning reads as a scam.
        Assert.Equal("Allodia", AllodiaSubscriptionCard.BillerName(new AllodiaBiller.Allodia()));
        Assert.Equal("Apple", AllodiaSubscriptionCard.BillerName(new AllodiaBiller.Apple()));
        Assert.Equal("Google Play", AllodiaSubscriptionCard.BillerName(new AllodiaBiller.Google()));
        // A biller this build cannot name still has to appear, or a "you are being charged twice"
        // warning would name only one of the two.
        Assert.Equal(
            "Some Shop",
            AllodiaSubscriptionCard.BillerName(new AllodiaBiller.Unknown("Some Shop")));
    }

    [Fact]
    public void OnlyWhoIsChargingRightNowIsNamed()
    {
        // The service keeps a store subscription in the list after it ends, so an account that
        // bought at one store and later at another carries both. Naming the first named the dead
        // one, which shipped once already on another client.
        var subscription = AllodiaSubscriptionFixture.Stores(
            AllodiaSubscriptionFixture.Store(
                AllodiaStore.Apple, new AllodiaStoreStatus.Expired(), autoRenewing: false),
            AllodiaSubscriptionFixture.Store(AllodiaStore.Google));
        var billers = AllodiaSubscriptionCard.Billers(subscription)
            .Select(AllodiaSubscriptionCard.BillerName)
            .ToArray();
        Assert.Equal(["Google Play"], billers);
    }

    [Fact]
    public void OneStoreIsNamedOnceHoweverManySubscriptionsItHolds()
    {
        // An account carries two at one store the moment somebody resubscribes: the lapsed one is
        // still inside the period it was paid for. Naming it twice says somebody is charged twice
        // by it.
        var subscription = AllodiaSubscriptionFixture.Stores(
            AllodiaSubscriptionFixture.Store(
                AllodiaStore.Google, new AllodiaStoreStatus.Cancelled(), autoRenewing: false),
            AllodiaSubscriptionFixture.Store(AllodiaStore.Google));
        Assert.Single(AllodiaSubscriptionCard.Billers(subscription));
        Assert.Single(AllodiaSubscriptionCard.ManageableStores(subscription));
    }

    [Fact]
    public void AStatusThisBuildCannotNameIsNeverReadAsPermission()
    {
        var unknown = new AllodiaStoreStatus.Unknown("something_new");
        Assert.False(AllodiaSubscriptionCard.StoreIsBilling(unknown));
        Assert.False(AllodiaSubscriptionCard.StoreCanBeManaged(unknown));
    }

    [Fact]
    public void BillingAndManageableDifferOnExactlyTheStatesThatMatterMost()
    {
        // On hold and paused grant nothing, so neither may claim the "billed by" line; both are
        // fixed only at the store, so dropping their button strands the person it matters to.
        foreach (var status in new AllodiaStoreStatus[]
                 {
                     new AllodiaStoreStatus.OnHold(), new AllodiaStoreStatus.Paused(),
                 })
        {
            Assert.False(AllodiaSubscriptionCard.StoreIsBilling(status));
            Assert.True(AllodiaSubscriptionCard.StoreCanBeManaged(status));
        }
        // Expired and revoked are the other way about: nothing is left to manage, and offering the
        // route anyway walks somebody into the store's own resubscribe button.
        foreach (var status in new AllodiaStoreStatus[]
                 {
                     new AllodiaStoreStatus.Expired(), new AllodiaStoreStatus.Revoked(),
                 })
        {
            Assert.False(AllodiaSubscriptionCard.StoreCanBeManaged(status));
        }
    }

    [Fact]
    public void RenewingIsAskedOfTheSubscriptionAndNotOfEntitlement()
    {
        Assert.True(AllodiaSubscriptionCard.WillRenew(AllodiaSubscriptionFixture.Own()));
        // Cancelled: entitled until the period ends, and charging nothing after it, so the card
        // says "runs until" rather than "renews on".
        Assert.False(AllodiaSubscriptionCard.WillRenew(AllodiaSubscriptionFixture.Own(
            status: new AllodiaOwnStatus.Cancelled(), nextPayment: null)));
        Assert.False(AllodiaSubscriptionCard.WillRenew(AllodiaSubscriptionFixture.Stores(
            AllodiaSubscriptionFixture.Store(
                AllodiaStore.Apple, new AllodiaStoreStatus.Cancelled(), autoRenewing: false))));
    }

    [Fact]
    public void ARetriedChargeIsNotALapse()
    {
        // Access continues through both of these, so the card says a payment failed and never that
        // the subscription has stopped.
        Assert.True(AllodiaSubscriptionCard.InGrace(
            AllodiaSubscriptionFixture.Own(status: new AllodiaOwnStatus.PastDue())));
        Assert.True(AllodiaSubscriptionCard.InGrace(AllodiaSubscriptionFixture.Stores(
            AllodiaSubscriptionFixture.Store(AllodiaStore.Google, new AllodiaStoreStatus.Grace()))));
        Assert.False(AllodiaSubscriptionCard.InGrace(AllodiaSubscriptionFixture.Own()));
    }

    [Fact]
    public void WhatMayBeDoneIsTheServicesAnswerAndNotAReadingOfTheStatus()
    {
        // The service computed `actions` with every biller in view, which no client is in a
        // position to do: somebody the App Store is charging has a period, and switching it
        // through this API is exactly what the service refuses.
        var storeBilled = AllodiaSubscriptionFixture.Stores(
            AllodiaSubscriptionFixture.Store(AllodiaStore.Apple));
        Assert.Null(AllodiaSubscriptionCard.SwitchTarget(storeBilled));
        Assert.False(AllodiaSubscriptionCard.OffersRestart(storeBilled));

        // A monthly subscription of Allodia's own, which the service says may switch.
        Assert.Equal(
            AllodiaPlan.Yearly,
            AllodiaSubscriptionCard.SwitchTarget(AllodiaSubscriptionFixture.Own()));
        Assert.Equal(
            AllodiaPlan.Monthly,
            AllodiaSubscriptionCard.SwitchTarget(
                AllodiaSubscriptionFixture.Own(interval: AllodiaPlan.Yearly)));
        // And one it says may not, whatever period it is on: a subscription mid-retry after a
        // failed charge cannot change plan.
        Assert.Null(AllodiaSubscriptionCard.SwitchTarget(AllodiaSubscriptionFixture.Own(
            status: new AllodiaOwnStatus.PastDue(),
            actions: new AllodiaSubscriptionActions(true, false, false, false))));
    }

    [Fact]
    public void RestartingIsOfferedOnlyWhenTheServiceSaysItWouldWork()
    {
        var cancelled = AllodiaSubscriptionFixture.Own(
            status: new AllodiaOwnStatus.Cancelled(),
            nextPayment: null,
            actions: new AllodiaSubscriptionActions(false, true, false, false));
        Assert.True(AllodiaSubscriptionCard.OffersRestart(cancelled));
        // Cancelled, but the service will not restart it, because the period has run out and what
        // is needed is a new checkout. Only it knows which.
        var lapsed = AllodiaSubscriptionFixture.Own(
            status: new AllodiaOwnStatus.Cancelled(),
            nextPayment: null,
            actions: AllodiaSubscriptionFixture.NoActions());
        Assert.False(AllodiaSubscriptionCard.OffersRestart(lapsed));
        // Running, so there is nothing to restart, whatever `canResubscribe` happens to say.
        Assert.False(AllodiaSubscriptionCard.OffersRestart(AllodiaSubscriptionFixture.Own(
            actions: new AllodiaSubscriptionActions(true, true, true, false))));
    }

    [Fact]
    public void BothConfirmationsNameTheDayAlreadyPaidFor()
    {
        // ⚠️ Without it the cancellation reads as "it stops now", which is the one thing it does
        // not do.
        Assert.Equal(
            AllodiaSubscriptionFixture.PeriodEnd,
            AllodiaSubscriptionCard.WriteDate(AllodiaSubscriptionFixture.Own()));
        // No date from either place is an empty string rather than a null the sentence would
        // render as "until ".
        Assert.Equal(string.Empty, AllodiaSubscriptionCard.WriteDate(
            AllodiaSubscriptionFixture.Free()));
    }
}
