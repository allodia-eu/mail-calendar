// What Settings → Writing style does with the core's records (docs/ai.md rules 8 to 16). The page,
// the flow and every control's state are the core's, pinned in mailcal-app and mailcal-bindings;
// what is pinned here is what the client words and keeps: a refusal's words, a name's Save, the
// run's line, a range's label, a day, a language, and an outcome said once.

using System.Globalization;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class WritingStyleRulesTests
{
    private static readonly ControlState Free = new(true, false);
    private static readonly ControlState Running = new(false, true);

    private static readonly WritingStyleWords Words = new(
        "give it a name", "gone", "learning…", (done, total) => $"learning ({done} of {total})…",
        "last twelve months", "everything", "until last year", "between", "fetches older", "fetches all",
        "mostly", "often", "sometimes");

    private static WritingStylePage Page(WritingStyleOutcome? outcome = null) =>
        new([], [], Free, null, null, outcome);

    private static WritingStyleOutcome Outcome(ulong serial, WritingStyleResult result) =>
        new(serial, WritingStyleChangeKind.Rename, "style-1", null, result);

    [Fact]
    public void ARefusalIsSaidInItsOwnWords()
    {
        Assert.Equal("give it a name", WritingStyleRules.Say(WritingStyleRefusal.BlankName, Words));
        Assert.Equal("gone", WritingStyleRules.Say(WritingStyleRefusal.Gone, Words));
    }

    [Fact]
    public void ANameSaveIsHeldWhileTheCoreRefusesTheNameAndRunsAsTheRecordSays()
    {
        Assert.Equal(Free, WritingStyleRules.NameSave(Free, null));
        Assert.Equal(new ControlState(false, false), WritingStyleRules.NameSave(Free, WritingStyleRefusal.BlankName));
        Assert.Equal(Running, WritingStyleRules.NameSave(Running, null));
    }

    private static readonly LetterWords Letter = new(
        greeting => $"opens with {greeting}", count => $"{count} words", count => $"{count} paragraphs",
        signOff => $"signs off with {signOff}", name => $"signed {name}");

    private static WritingStyleParagraph Lines(int count) => new(Enumerable.Repeat(0.8, count).ToArray());

    [Fact]
    public void TheLetterIsSpokenInOrderWithAPlaceholderAsItsWordAndTheRecordsParagraphCount()
    {
        var letter = new WritingStyleLetter(
            [new WritingStyleSpan("Hi ", false), new WritingStyleSpan("Name", true), new WritingStyleSpan(",", false)],
            [Lines(3), Lines(2), Lines(4)],
            [new WritingStyleSpan("Best,", false)],
            "Eva", 70, 2, "Short paragraphs");
        Assert.Equal("opens with Hi Name, 70 words, 2 paragraphs, signs off with Best, signed Eva",
            WritingStyleRules.LetterSpoken(letter, Letter));
    }

    // The joining comma stands in for the mark a greeting or sign-off ends on; one ending on none,
    // or on another mark, is read as written.
    [Fact]
    public void AGreetingOrSignOffIsSpokenWithoutTheCommaSemicolonColonOrFullStopItEndsOn()
    {
        Assert.Equal("opens with Dear Bob, signs off with Kind regards, signed Eva",
            WritingStyleRules.LetterSpoken(Greeted("Dear Bob: ", "Kind regards."), Letter));
        Assert.Equal("opens with Hello all, signs off with Thanks",
            WritingStyleRules.LetterSpoken(Greeted("Hello all;", "Thanks", ""), Letter));
        Assert.Equal("opens with Hi Bob, signs off with Cheers!, signed Eva",
            WritingStyleRules.LetterSpoken(Greeted("Hi Bob", "Cheers!"), Letter));
    }

    private static WritingStyleLetter Greeted(string opening, string closing, string name = "Eva") =>
        new([new WritingStyleSpan(opening, false)], [Lines(2)], [new WritingStyleSpan(closing, false)], name, 0, 0, "");

    [Fact]
    public void WhatTheLetterDoesNotKnowIsLeftOutOfWhatIsSpoken()
    {
        var letter = new WritingStyleLetter([], [Lines(2), Lines(2)], [], "", 0, 0, "");
        Assert.Equal(string.Empty, WritingStyleRules.LetterSpoken(letter, Letter));
        var signed = letter with { SignsAs = "Eva", Words = 40 };
        Assert.Equal("40 words, signed Eva", WritingStyleRules.LetterSpoken(signed, Letter));
    }

    // Zero is "not known yet": a fraction then would read as nothing done of nothing.
    [Fact]
    public void TheRunSaysNoFractionUntilItKnowsItsTotal()
    {
        Assert.Equal("learning…", WritingStyleRules.Progress(0, 0, Words));
        Assert.Equal("learning (2 of 5)…", WritingStyleRules.Progress(2, 5, Words));
    }

    [Fact]
    public void EachRangeIsLabelledByTheChoiceItIs()
    {
        Assert.Equal("last twelve months", WritingStyleRules.Range(new StyleLearnRange.LastTwelveMonths(), Words));
        Assert.Equal("everything", WritingStyleRules.Range(new StyleLearnRange.Everything(), Words));
        Assert.Equal("until last year", WritingStyleRules.Range(new StyleLearnRange.UntilEndOfLastYear(), Words));
        Assert.Equal("between", WritingStyleRules.Range(new StyleLearnRange.Between("2026-01-01", "2026-03-31"), Words));
    }

    [Fact]
    public void WhatARangeFetchesIsOneLineAndNothingFetchedIsNone()
    {
        Assert.Null(WritingStyleRules.Fetch(StyleRangeFetch.Nothing, Words));
        Assert.Equal("fetches older", WritingStyleRules.Fetch(StyleRangeFetch.Older, Words));
        Assert.Equal("fetches all", WritingStyleRules.Fetch(StyleRangeFetch.WholeHistory, Words));
    }

    [Fact]
    public void AFrequencyIsWordedAsTheCoreGraded()
    {
        Assert.Equal("mostly", WritingStyleRules.Frequency(WritingStyleFrequency.Mostly, Words));
        Assert.Equal("often", WritingStyleRules.Frequency(WritingStyleFrequency.Often, Words));
        Assert.Equal("sometimes", WritingStyleRules.Frequency(WritingStyleFrequency.Sometimes, Words));
    }

    [Fact]
    public void ARecordsDayReadsAndWritesAsTheCoreWritesIt()
    {
        Assert.Equal(new DateOnly(2026, 11, 19), WritingStyleRules.Day("2026-11-19"));
        Assert.Null(WritingStyleRules.Day("19-11-2026"));
        Assert.Equal("2026-02-03", WritingStyleRules.Day(new DateOnly(2026, 2, 3)));
    }

    // A moment late in the evening in UTC is the next day in Amsterdam.
    [Fact]
    public void AMomentIsTheDayItFallsOnInTheDisplayZone()
    {
        var seconds = new DateTimeOffset(2026, 11, 19, 23, 30, 0, TimeSpan.Zero).ToUnixTimeSeconds();
        var amsterdam = TimeZoneInfo.FindSystemTimeZoneById("Europe/Amsterdam");
        Assert.Equal(new DateOnly(2026, 11, 20), WritingStyleRules.DayOf(seconds, amsterdam));
        Assert.Equal(new DateOnly(2026, 11, 19), WritingStyleRules.DayOf(seconds, TimeZoneInfo.Utc));
    }

    [Fact]
    public void ALanguageIsNamedByTheCatalogThenTheCultureThenItsCode()
    {
        string Catalog(string code) => code == "nl" ? "Nederlands" : "settings_language_" + code;
        Assert.Equal("Nederlands", WritingStyleRules.LanguageName("nl", Catalog));
        Assert.Equal(CultureInfo.GetCultureInfo("sv").NativeName, WritingStyleRules.LanguageName("sv", Catalog));
        Assert.Equal("zz", WritingStyleRules.LanguageName("zz", Catalog));
    }

    [Fact]
    public void LearnIsToldTheLanguageTheAppIsReadIn()
    {
        string[] locales = ["en", "nl", "de"];
        Assert.Equal("nl", WritingStyleRules.AppLanguage("nl", "de", locales));
        Assert.Equal("de", WritingStyleRules.AppLanguage("system", "de", locales));
        Assert.Equal("en", WritingStyleRules.AppLanguage("system", "sv", locales));
    }

    [Fact]
    public void AnOutcomeIsActedOnOnce()
    {
        var outcomes = new WritingStyleOutcomes(Page());
        var refused = Outcome(1, new WritingStyleResult.Refused(WritingStyleRefusal.BlankName));
        Assert.Same(refused, outcomes.Fresh(refused));
        Assert.Null(outcomes.Fresh(refused));
        Assert.NotNull(outcomes.Fresh(Outcome(2, new WritingStyleResult.Done())));
    }

    // Settings opened again starts past what the page already carries.
    [Fact]
    public void AnOutcomeFromBeforeSettingsOpenedIsNotActedOn()
    {
        var earlier = Outcome(7, new WritingStyleResult.Refused(WritingStyleRefusal.Gone));
        var outcomes = new WritingStyleOutcomes(Page(earlier));
        Assert.Null(outcomes.Fresh(earlier));
    }
}
