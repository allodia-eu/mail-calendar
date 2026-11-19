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
        "everything", "until last year", "between", "mostly", "often", "sometimes");

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
        Assert.Equal("everything", WritingStyleRules.Range(new StyleLearnRange.Everything(), Words));
        Assert.Equal("until last year", WritingStyleRules.Range(new StyleLearnRange.UntilEndOfLastYear(), Words));
        Assert.Equal("between", WritingStyleRules.Range(new StyleLearnRange.Between("2026-01-01", "2026-03-31"), Words));
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
