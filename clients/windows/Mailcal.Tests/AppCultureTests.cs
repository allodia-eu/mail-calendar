// The app's Language choice must drive its DATES, not just its words (docs/timestamps.md).
//
// The bug this pins: LanguageStore applied the choice only as an MRT-Core resource qualifier, so
// the chrome went Dutch while every date kept formatting against the host's regional format. A
// fully Dutch UI listed "Mon"/"Sun" and the calendar was headed "Jul 2026 · Mon Tue Wed", visible
// in the nl store screenshots before it was visible anywhere else.
//
// Note what could NOT have caught it: RelativeDateTests asserts the weekday label equals
// `ToString("ddd", CultureInfo.CurrentCulture)`, which is true in every culture, including a wrong
// one. Hence the culture-injectable overload, these assert against the *chosen* culture instead.
//
// These compare the label to the culture's own name for the day/month rather than to a literal
// "ma" / "3 mrt", because CLDR revises Dutch abbreviations between ICU versions (the same trap
// AGENTS.md records for Robolectric: `jul` on one JDK, `jul.` on another). Asserting the copy is
// *Dutch*, not *which* Dutch, is what keeps this from breaking on a toolchain bump. Each test first
// asserts the two languages disagree at all, so it cannot silently degrade into a tautology.

using System;
using System.Globalization;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AppCultureTests
{
    private static readonly CultureInfo Nl = AppCulture.Resolve("nl")!;
    private static readonly CultureInfo En = AppCulture.Resolve("en")!;

    private static readonly DateTimeOffset Now =
        DateTimeOffset.Parse("2026-07-21T20:00:00Z", CultureInfo.InvariantCulture);

    [Theory]
    [InlineData("nl", "nl")]
    [InlineData("en", "en")]
    [InlineData("de", "de")]
    [InlineData("fr", "fr")]
    [InlineData("es", "es")]
    [InlineData("it", "it")]
    [InlineData("pt", "pt")]
    public void AnExplicitChoiceResolvesToThatLanguage(string choice, string expected) =>
        Assert.Equal(expected, AppCulture.Resolve(choice)?.Name);

    // Every catalog locale resolves, the picker offers exactly this list, and one it could offer
    // but this method did not know would render its own words over the host's dates.
    [Fact]
    public void EveryCatalogLocaleResolves() =>
        Assert.All(L10n.Locales, locale => Assert.Equal(locale, AppCulture.Resolve(locale)?.Name));

    // "system" leaves the host's regional format alone, on Windows that is a separate setting from
    // the display language, and it is the user's own stated formatting preference. So does a
    // language this build does not ship.
    [Theory]
    [InlineData("system")]
    [InlineData("")]
    [InlineData("fi")]
    public void SystemAndUnknownChoicesFollowTheHost(string choice) =>
        Assert.Null(AppCulture.Resolve(choice));

    // The regression proper: the same instant, the same zone, the same bucket, only the app's
    // language differs, and the weekday must follow it. Fails on the old code, which ignored the
    // choice and rendered whatever the host called that day.
    [Fact]
    public void TheWeekdayBucketFollowsTheAppLanguage()
    {
        const string Monday = "2026-07-20T09:05:00Z"; // one day back -> the weekday bucket
        var nlName = Nl.DateTimeFormat.AbbreviatedDayNames[(int)DayOfWeek.Monday];
        var enName = En.DateTimeFormat.AbbreviatedDayNames[(int)DayOfWeek.Monday];
        Assert.NotEqual(nlName, enName); // the premise: Dutch names this day differently

        Assert.Equal(nlName, TimeZones.RelativeDate(Monday, "UTC", Now, Nl));
        Assert.Equal(enName, TimeZones.RelativeDate(Monday, "UTC", Now, En));
    }

    // The other two buckets carry a month name, for the same reason.
    [Fact]
    public void TheMonthBucketsFollowTheAppLanguage()
    {
        const int March = 3;
        var nlMonth = Nl.DateTimeFormat.AbbreviatedMonthNames[March - 1];
        var enMonth = En.DateTimeFormat.AbbreviatedMonthNames[March - 1];
        Assert.NotEqual(nlMonth, enMonth);

        // This year, older than six days -> day + month.
        Assert.Equal($"3 {nlMonth}", TimeZones.RelativeDate("2026-03-03T09:05:00Z", "UTC", Now, Nl));
        Assert.Equal($"3 {enMonth}", TimeZones.RelativeDate("2026-03-03T09:05:00Z", "UTC", Now, En));

        // A previous year -> day + month + year.
        Assert.Equal($"3 {nlMonth} 2024", TimeZones.RelativeDate("2024-03-03T09:05:00Z", "UTC", Now, Nl));
        Assert.Equal($"3 {enMonth} 2024", TimeZones.RelativeDate("2024-03-03T09:05:00Z", "UTC", Now, En));
    }

    // Today's bucket is the 24-hour clock, which is language-independent, it must NOT drift when
    // the culture changes (a Dutch culture would otherwise be free to re-punctuate it).
    [Fact]
    public void TheClockBucketIsIdenticalInBothLanguages()
    {
        const string Today = "2026-07-21T09:05:00Z";

        Assert.Equal("09:05", TimeZones.RelativeDate(Today, "UTC", Now, Nl));
        Assert.Equal("09:05", TimeZones.RelativeDate(Today, "UTC", Now, En));
    }
}
