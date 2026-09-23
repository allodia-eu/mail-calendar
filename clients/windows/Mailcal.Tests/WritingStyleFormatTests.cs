// The numbers, dates, language names and reveal fields the Writing style screens put on screen.
// Each is a rule a screenshot reads straight past: a balance printed with three decimals, a
// description written in a language the person does not read, a reveal drawing a heading over an
// empty field, or a language named by its code.

using System;
using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
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

    [Fact]
    public void AHabitIsItsWordingThenItsShare() =>
        Assert.Equal("Hi Anna, (40 %)", StyleReveal.Habit(new HabitRow("Hi Anna,", 40), Invariant));

    // In the order every client draws, and a field with nothing in it is not drawn at all.
    [Fact]
    public void TheRevealDrawsOnlyFieldsThatSaySomething()
    {
        var language = new LanguageStyleRow(
            Language: "en",
            Greetings: new[] { new HabitRow("Hi,", 60), new HabitRow(" ", 10) },
            SignOffs: Array.Empty<HabitRow>(),
            SignsAs: "Anna",
            Register: "",
            TypicalWords: 0,
            Shape: "Short paragraphs",
            Punctuation: "  ",
            Structure: "",
            Moves: "",
            Phrases: new[] { "no worries", "" },
            Avoid: Array.Empty<string>());
        var sections = StyleReveal.Sections(language, Invariant);
        Assert.Equal(
            new[] { RevealField.Greetings, RevealField.SignsAs, RevealField.Shape, RevealField.Phrases },
            sections.Select(section => section.Field).ToArray());
        Assert.Equal(new[] { "Hi, (60 %)" }, sections[0].Lines);
        Assert.Equal(new[] { "no worries" }, sections[3].Lines);
    }

    // The length is a number in a sentence of its own, placed after the register.
    [Fact]
    public void ATypicalLengthIsDrawnWhenKnown()
    {
        var language = new LanguageStyleRow(
            Language: "nl",
            Greetings: Array.Empty<HabitRow>(),
            SignOffs: Array.Empty<HabitRow>(),
            SignsAs: "",
            Register: "Informal",
            TypicalWords: 120,
            Shape: "",
            Punctuation: "",
            Structure: "",
            Moves: "",
            Phrases: Array.Empty<string>(),
            Avoid: new[] { "exclamation marks" });
        var sections = StyleReveal.Sections(language, Invariant);
        Assert.Equal(
            new[] { RevealField.Register, RevealField.Length, RevealField.Avoid },
            sections.Select(section => section.Field).ToArray());
        Assert.Equal(120, sections[1].Words);
    }
}
