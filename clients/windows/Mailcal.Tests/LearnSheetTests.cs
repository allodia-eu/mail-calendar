// The learn sheet's rules (docs/ai.md, "Learning"). Consent is given against the report on screen,
// so a report for a choice the person has since changed must not stand for the new one; and "up to
// a date" has to take in that whole day, or the last day's mail is left out with nothing to say so.

using System;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class LearnSheetTests
{
    private static readonly TimeZoneInfo Amsterdam = TimeZoneInfo.FindSystemTimeZoneById("Europe/Amsterdam");

    private static CorpusReport Report(uint usable) => new(
        Found: 12,
        Usable: usable,
        Undetected: 0,
        Languages: Array.Empty<CorpusLanguage>(),
        Oldest: null,
        Newest: null,
        Horizon: null);

    private static LearnSheet Sheet(int accounts = 1) => new("work", new DateOnly(2024, 12, 31), accounts);

    [Theory]
    [InlineData(0, false)]
    [InlineData(1, false)]
    [InlineData(2, true)]
    public void TheAccountIsAskedOnlyWhenThereIsAChoice(int accounts, bool asks) =>
        Assert.Equal(asks, LearnSheet.AsksForAccount(accounts));

    // An account page only when there is a choice, and the run last, one page each.
    [Fact]
    public void TheSheetsPagesFollowTheAccounts()
    {
        Assert.Equal(
            new[] { LearnStep.Range, LearnStep.Consent, LearnStep.Progress },
            Sheet().Steps);
        var sheet = Sheet(accounts: 3);
        Assert.Equal(
            new[] { LearnStep.Account, LearnStep.Range, LearnStep.Consent, LearnStep.Progress },
            sheet.Steps);
        Assert.Equal(4, sheet.Pager.Count);
        Assert.Equal(LearnStep.Account, sheet.Step);
        sheet.Pager.Go(2);
        Assert.Equal(LearnStep.Consent, sheet.Step);
    }

    [Fact]
    public void EverythingOnTheDeviceHasNoEnd() => Assert.Null(Sheet().Until(Amsterdam));

    // The core's range is inclusive, so the day ends one second before the next one begins, in
    // the zone the dates are shown in, winter and summer alike.
    [Fact]
    public void UpToADateTakesInTheWholeDay()
    {
        Assert.Equal(1735685999L, LearnSheet.EndOfDay(new DateOnly(2024, 12, 31), Amsterdam));
        Assert.Equal(1722463199L, LearnSheet.EndOfDay(new DateOnly(2024, 7, 31), Amsterdam));
        var sheet = Sheet();
        sheet.UpToDate = true;
        Assert.Equal(1735685999L, sheet.Until(Amsterdam));
    }

    // Two reads in flight for two choices: only the later one may be drawn, whichever lands first.
    [Fact]
    public void AReadForAChoiceSinceChangedIsDropped()
    {
        var sheet = Sheet();
        var first = sheet.BeginRead();
        var second = sheet.BeginRead();
        Assert.False(sheet.Finish(first, Report(usable: 5), null));
        Assert.True(sheet.Reading);
        Assert.Null(sheet.Report);
        Assert.True(sheet.Finish(second, Report(usable: 0), null));
        Assert.False(sheet.Reading);
        Assert.Equal(0u, sheet.Report?.Usable);
    }

    [Fact]
    public void ConsentIsOfferedOnlyOverSomethingToLearnFrom()
    {
        var sheet = Sheet();
        Assert.False(sheet.CanLearn);
        var read = sheet.BeginRead();
        Assert.False(sheet.CanLearn);
        sheet.Finish(read, Report(usable: 0), null);
        Assert.False(sheet.CanLearn);
        read = sheet.BeginRead();
        sheet.Finish(read, null, new AiFailure(AiProblem.NoSentFolder));
        Assert.False(sheet.CanLearn);
        read = sheet.BeginRead();
        sheet.Finish(read, Report(usable: 3), null);
        Assert.True(sheet.CanLearn);
    }

    // A new read takes the old answer off the sheet, so consent cannot be given against it while
    // the new one is on its way.
    [Fact]
    public void ANewReadWithdrawsTheLastAnswer()
    {
        var sheet = Sheet();
        sheet.Finish(sheet.BeginRead(), Report(usable: 3), null);
        Assert.True(sheet.CanLearn);
        sheet.BeginRead();
        Assert.False(sheet.CanLearn);
        Assert.Null(sheet.Report);
    }
}
