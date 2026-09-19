// The dates and amounts the subscription card puts in front of somebody (`purchasing.md`).
//
// Both of these fail as sentences rather than as errors. A date restated in another zone drifts by
// a day, so "you keep everything until 11 October" becomes "until 10 October" for a promise about
// somebody's money, and the string it was read from still says the eleventh; a shape this build
// cannot parse must still say the right day rather than nothing; and a currency code this host has
// no region for must price as a number instead of throwing, because a card that threw over a code
// would say nothing at all.

using System;
using System.Globalization;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AllodiaSubscriptionFormatTests
{
    private static readonly CultureInfo English = new("en-GB");
    private static readonly CultureInfo Dutch = new("nl-NL");

    [Fact]
    public void AnOffsetBearingInstantIsStatedAsTheDayItNames()
    {
        // ⚠️ The account service is not the sync engine and sends a numeric offset rather than the
        // engine's `Z`, so this is the ordinary shape rather than an edge case.
        Assert.Equal(
            "11 October 2026",
            AllodiaSubscriptionFormat.Date("2026-10-11T00:00:00+02:00", English));
        Assert.Equal(
            "11 oktober 2026",
            AllodiaSubscriptionFormat.Date("2026-10-11T00:00:00+02:00", Dutch));
    }

    [Fact]
    public void TheDayIsTheOneTheServiceWroteAndNotOneRestatedInAnotherZone()
    {
        // ⚠️ The rule that keeps the parsed path and the ten-character fallback agreeing. A period
        // ending at midnight +02:00 restated in UTC is the day before, so a build that could parse
        // the string would name a different day from one that could not, about somebody's money,
        // and neither would look wrong.
        const string Midnight = "2026-10-11T00:00:00+02:00";
        Assert.Equal("11 October 2026", AllodiaSubscriptionFormat.Date(Midnight, English));
        Assert.StartsWith("2026-10-11", Midnight, StringComparison.Ordinal);
    }

    [Fact]
    public void TheWeekdayIsLeftOffTheDateEveryClientStates()
    {
        // .NET's "D" carries one and the two twins' long styles do not, so taking it would put a
        // different sentence on this one client for no reason anybody chose.
        Assert.DoesNotContain("Sunday", AllodiaSubscriptionFormat.Date("2026-10-11", English));
        Assert.DoesNotContain("zondag", AllodiaSubscriptionFormat.Date("2026-10-11", Dutch));
    }

    [Fact]
    public void ADayWithNoTimeAtAllIsTheOtherShapeTheServiceSends()
    {
        Assert.Equal("11 October 2026", AllodiaSubscriptionFormat.Date("2026-10-11", English));
    }

    [Fact]
    public void AShapeThisBuildCannotReadStillSaysTheRightDay()
    {
        // The leading ten characters are the day whatever follows them, so the sentence stays true
        // and only stops being localised. The same fallback Apple's and Android's cards take.
        Assert.Equal(
            "2026-10-11",
            AllodiaSubscriptionFormat.Date("2026-10-11 what the service invented next", English));
    }

    [Fact]
    public void NoDateIsNullRatherThanASentenceEndingInNothing()
    {
        Assert.Null(AllodiaSubscriptionFormat.Date(null, English));
        Assert.Null(AllodiaSubscriptionFormat.Date(string.Empty, English));
    }

    [Fact]
    public void MinorUnitsArePricedInTheCurrencyTheServiceNamed()
    {
        // The account is billed in what the service says, which is not necessarily what the
        // reader's own region spends: a Dutch reader on a pound subscription must see pounds.
        Assert.Equal("€4.99", AllodiaSubscriptionFormat.MinorUnits(499, "EUR", English));
        Assert.Equal("£49.99", AllodiaSubscriptionFormat.MinorUnits(4999, "GBP", English));
        Assert.Contains("£", AllodiaSubscriptionFormat.MinorUnits(4999, "GBP", Dutch));
    }

    [Fact]
    public void ACurrencyThisHostCannotNamePricesAsItsCodeRatherThanThrowing()
    {
        // An amount with no symbol is still the right amount; a card that threw over a code would
        // say nothing at all.
        var priced = AllodiaSubscriptionFormat.MinorUnits(12345, "XTS", English);
        Assert.Contains("123.45", priced);
        Assert.Contains("XTS", priced);
    }

    [Fact]
    public void AnAmountWithNoCurrencyBesideItIsStillAnAmount()
    {
        // ⚠️ `AllodiaIntervalChange` carries minor units alone, so the card prices it from the
        // subscription's own currency; when there is not one, the number alone beats nothing.
        Assert.Equal("49.99", AllodiaSubscriptionFormat.MinorUnits(4999, null, English));
        Assert.Equal("49,99", AllodiaSubscriptionFormat.MinorUnits(4999, "  ", Dutch));
    }
}
