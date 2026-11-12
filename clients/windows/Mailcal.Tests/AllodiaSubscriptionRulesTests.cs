// How the subscription section words what the core decided (AllodiaSubscriptionCard). Which state,
// control and outcome it carries is the core's, pinned in mailcal-bindings'
// allodia_subscription_facts_tests.rs and allodia_subscription_card_tests.rs.

using System.Globalization;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;
using static Allodia.Mailcal.Tests.AllodiaSubscriptionFixture;

namespace Allodia.Mailcal.Tests;

public class AllodiaSubscriptionRulesTests
{
    private static readonly CultureInfo English = CultureInfo.GetCultureInfo("en-GB");

    private static readonly AllodiaSubscriptionWords Words = new(
        date => $"cancelled, yours until {date}",
        "running again",
        (date, amount) => $"next {amount} on {date}",
        "nothing changed",
        "already cancelled",
        "already active",
        "already on it",
        "not switchable",
        "not found");

    [Fact]
    public void AllodiaIsNamedAllodiaAndNotThePaymentProcessorBehindIt()
    {
        Assert.Equal("Allodia", AllodiaSubscriptionRules.BillerName(new AllodiaBiller.Allodia()));
        Assert.Equal("Apple", AllodiaSubscriptionRules.BillerName(new AllodiaBiller.Apple()));
        Assert.Equal("Google Play", AllodiaSubscriptionRules.BillerName(new AllodiaBiller.Google()));
    }

    [Fact]
    public void ABillerThisBuildCannotNameIsStillNamed()
    {
        // A duplicate-billing warning naming only one of the two would say nothing is wrong.
        Assert.Equal("Paddle", AllodiaSubscriptionRules.BillerName(new AllodiaBiller.Unknown("Paddle")));
    }

    [Fact]
    public void AStorePageIsNamedAfterItsStore()
    {
        Assert.Equal("Apple", AllodiaSubscriptionRules.StoreName(AllodiaStore.Apple));
        Assert.Equal("Google Play", AllodiaSubscriptionRules.StoreName(AllodiaStore.Google));
    }

    [Fact]
    public void OnlyASectionWithAnAnswerDrawnAsksAgainOnAReturn()
    {
        Assert.True(AllodiaSubscriptionRules.WorthRereading(new AllodiaSubscriptionState.Unavailable()));
        Assert.True(AllodiaSubscriptionRules.WorthRereading(new AllodiaSubscriptionState.NeedsReauth()));
        Assert.True(AllodiaSubscriptionRules.WorthRereading(new AllodiaSubscriptionState.Loaded(View())));
        // A read still out answers for the return, and a hidden section has nobody to ask about.
        Assert.False(AllodiaSubscriptionRules.WorthRereading(new AllodiaSubscriptionState.Checking()));
        Assert.False(AllodiaSubscriptionRules.WorthRereading(new AllodiaSubscriptionState.Hidden()));
    }

    [Fact]
    public void APaymentPageIsOpenedOncePerId()
    {
        var card = Card(new AllodiaSubscriptionState.Loaded(View()), page: new AllodiaCheckoutPage(4, "https://pay.example.test/s/4"));
        Assert.Equal(4UL, AllodiaSubscriptionRules.CheckoutPageToOpen(card, 3)?.Id);
        Assert.Null(AllodiaSubscriptionRules.CheckoutPageToOpen(card, 4));
        Assert.Null(AllodiaSubscriptionRules.CheckoutPageToOpen(Loaded(View()), 0));
    }

    [Fact]
    public void EachBuyButtonCarriesItsOwnPlansControl()
    {
        var running = new ControlState(false, true);
        var held = new ControlState(false, false);
        var checkout = new AllodiaCheckoutOffer(Prices(), Monthly: held, Yearly: running);
        Assert.Equal(running, AllodiaSubscriptionRules.CheckoutControl(checkout, AllodiaPlan.Yearly));
        Assert.Equal(held, AllodiaSubscriptionRules.CheckoutControl(checkout, AllodiaPlan.Monthly));
    }

    [Fact]
    public void AStorePageAppearingOnItsOwnIsAChangeToDraw()
    {
        // A record's own text names an array by its type, so without the lists spelled out this
        // would compare equal and the page would never be drawn.
        var before = Loaded(View());
        var after = Loaded(View(storePages: [new AllodiaStorePage(AllodiaStore.Google, "https://play.example.test")]));
        Assert.NotEqual(AllodiaSubscriptionRules.Fingerprint(before), AllodiaSubscriptionRules.Fingerprint(after));
        var duplicate = Loaded(View(duplicate: [new AllodiaBiller.Allodia(), new AllodiaBiller.Apple()]));
        Assert.NotEqual(AllodiaSubscriptionRules.Fingerprint(before), AllodiaSubscriptionRules.Fingerprint(duplicate));
        Assert.Equal(AllodiaSubscriptionRules.Fingerprint(before), AllodiaSubscriptionRules.Fingerprint(Loaded(View())));
    }

    [Fact]
    public void ARunningReadIsAChangeToDraw()
    {
        Assert.NotEqual(
            AllodiaSubscriptionRules.Fingerprint(Loaded(View())),
            AllodiaSubscriptionRules.Fingerprint(Loaded(View(refreshing: true))));
    }

    [Fact]
    public void ACancellationNamesTheDayAccessRunsTo()
    {
        Assert.Equal(
            "cancelled, yours until 11 October 2026",
            AllodiaSubscriptionRules.OutcomeText(new AllodiaSubscriptionOutcome.Cancelled(PeriodEnd), Words, English));
        // A day this build cannot read is still said: a cancellation with no date reads as "it
        // stops now", which is the one thing it does not do.
        Assert.Equal(
            "cancelled, yours until 2026-10-11",
            AllodiaSubscriptionRules.OutcomeText(new AllodiaSubscriptionOutcome.Cancelled("2026-10-11 sometime"), Words, English));
    }

    [Fact]
    public void ASwitchSaysTheServicesOwnFiguresInItsOwnCurrency()
    {
        Assert.Equal(
            "next €49.99 on 11 October 2026",
            AllodiaSubscriptionRules.OutcomeText(
                new AllodiaSubscriptionOutcome.Switched(PeriodEnd, 4999, "EUR"), Words, English));
    }

    [Fact]
    public void ARestartSaysItIsRunningAgain()
    {
        Assert.Equal(
            "running again",
            AllodiaSubscriptionRules.OutcomeText(new AllodiaSubscriptionOutcome.Restarted(), Words, English));
    }

    // One [Fact] over the pairs rather than a [Theory]: the generated enum is `internal`, so it
    // cannot cross a public test signature (CS0051).
    [Fact]
    public void EachRefusalEarnsItsOwnSentence()
    {
        var expected = new[]
        {
            (AllodiaWriteFailure.AlreadyCancelled, "already cancelled"),
            (AllodiaWriteFailure.AlreadyActive, "already active"),
            (AllodiaWriteFailure.AlreadyOnInterval, "already on it"),
            (AllodiaWriteFailure.NotSwitchable, "not switchable"),
            (AllodiaWriteFailure.NotFound, "not found"),
            (AllodiaWriteFailure.Unexplained, "nothing changed"),
        };
        foreach (var (failure, sentence) in expected)
        {
            Assert.Equal(
                sentence,
                AllodiaSubscriptionRules.OutcomeText(new AllodiaSubscriptionOutcome.Failed(failure), Words, English));
        }
    }
}
