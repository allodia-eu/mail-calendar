// The numbers, dates and language names the Writing style screens put on screen. Each is a rule a
// screenshot reads straight past: a balance printed with three decimals, a description written in
// a language the person does not read, or a language named by its code.

using System;
using System.Globalization;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class WritingStyleFormatTests
{
    private static readonly CultureInfo Invariant = CultureInfo.InvariantCulture;

    private static long Unix(string instant) =>
        DateTimeOffset.Parse(instant, Invariant).ToUnixTimeSeconds();

    // Grouped as the language groups, with at most one decimal.
    [Fact]
    public void CreditsKeepAtMostOneDecimal()
    {
        Assert.Equal("12", WritingStyleFormat.Credits(12.0, Invariant));
        Assert.Equal("7.5", WritingStyleFormat.Credits(7.46, Invariant));
        Assert.Equal("1,234.6", WritingStyleFormat.Credits(1234.56, Invariant));
        Assert.Equal("1.234,6", WritingStyleFormat.Credits(1234.56, CultureInfo.GetCultureInfo("nl")));
    }

    [Fact]
    public void ABalanceFromTodayShowsTheClock()
    {
        var now = DateTimeOffset.Parse("2026-07-20T20:00:00Z", Invariant);
        Assert.Equal(
            "09:05",
            WritingStyleFormat.AsOf(Unix("2026-07-20T09:05:00Z"), TimeZoneInfo.Utc, now, Invariant));
    }

    [Fact]
    public void AnOlderBalanceShowsTheDayAndTheYearOnlyWhenItDiffers()
    {
        var now = DateTimeOffset.Parse("2026-07-20T20:00:00Z", Invariant);
        Assert.Equal(
            "15 Jul",
            WritingStyleFormat.AsOf(Unix("2026-07-15T09:05:00Z"), TimeZoneInfo.Utc, now, Invariant));
        Assert.Equal(
            "15 Jul 2025",
            WritingStyleFormat.AsOf(Unix("2025-07-15T09:05:00Z"), TimeZoneInfo.Utc, now, Invariant));
    }

    // The day is the one in the zone the app shows dates in, not UTC's.
    [Fact]
    public void ADateIsTheDayInTheZone()
    {
        var amsterdam = TimeZoneInfo.FindSystemTimeZoneById("Europe/Amsterdam");
        Assert.Equal(
            "1 Jan 2025",
            WritingStyleFormat.Date(Unix("2024-12-31T23:30:00Z"), amsterdam, Invariant));
    }

    // Every language by its own name, and a code the catalog does not ship left as the code rather
    // than looked up into nothing.
    [Fact]
    public void LanguagesAreNamedByTheirEndonymWhenTheCatalogHasOne() =>
        Assert.Equal(
            "name:nl, name:en, sv",
            WritingStyleFormat.Languages(new[] { "nl", "en", "sv" }, code => "name:" + code));

    // The description is written in the language the app is shown in, so the person can read it.
    [Fact]
    public void TheDescriptionLanguageIsTheOneTheAppIsShownIn()
    {
        Assert.Equal("de", WritingStyleFormat.CatalogLocale("de", "nl"));
        Assert.Equal("nl", WritingStyleFormat.CatalogLocale("system", "nl"));
        Assert.Equal("en", WritingStyleFormat.CatalogLocale("system", "sv"));
    }
}
