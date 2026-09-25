// The reveal's steps and the arithmetic behind its pictures (docs/ai.md, "Learning" step 5). Each
// rule is one a page gets wrong while looking right: a letter whose lines ignore the paragraph
// count, a card that repeats its heading as its first sentence, a placeholder drawn with its
// brackets, a language control over a page about every language, and a style saved nameless.

using System;
using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class StyleRevealTests
{
    [Fact]
    public void TheLanguageControlIsOnTheFourPagesAboutOneLanguage()
    {
        Assert.Equal(6, StyleReveal.Steps.Count);
        Assert.Equal(
            new[] { RevealStep.Letter, RevealStep.Habits, RevealStep.Voice, RevealStep.Phrases },
            StyleReveal.Steps.Where(step => step.IsPerLanguage()).ToArray());
    }

    // A single language has nothing to choose between, and the pages about every language never
    // show the control.
    [Fact]
    public void TheLanguageIsChosenOnlyWhereThereIsAChoice()
    {
        var one = new RevealSheet("s", "Work", "", languages: 1);
        one.Pager.Go(1);
        Assert.False(one.ShowsLanguageChoice);
        var two = new RevealSheet("s", "Work", "", languages: 2);
        Assert.False(two.ShowsLanguageChoice);
        two.Pager.Go(1);
        Assert.True(two.ShowsLanguageChoice);
        two.Pager.Go(5);
        Assert.False(two.ShowsLanguageChoice);
    }

    [Fact]
    public void TheLanguageStaysWithinTheLanguagesThereAre()
    {
        var sheet = new RevealSheet("s", "Work", "", languages: 2);
        sheet.Language = 5;
        Assert.Equal(1, sheet.Language);
        sheet.Language = -1;
        Assert.Equal(0, sheet.Language);
        Assert.Equal(0, new RevealSheet("s", "Work", "", languages: 0) { Language = 3 }.Language);
    }

    [Fact]
    public void ThePagerStaysWithinItsPagesAndKnowsWhichWayItWent()
    {
        var pager = new WizardPager(6);
        Assert.True(pager.IsFirst && !pager.IsLast);
        Assert.False(pager.Go(-1));
        Assert.True(pager.Index == 0 && pager.Forward);
        Assert.True(pager.Go(5));
        Assert.True(pager.IsLast && pager.Forward);
        pager.Go(9);
        Assert.Equal(5, pager.Index);
        pager.Go(4);
        Assert.True(pager.Index == 4 && !pager.Forward);
        // Standing still is not a move back.
        Assert.False(pager.Go(4));
        Assert.False(pager.Forward);
        pager.Go(5);
        Assert.True(pager.Forward);
        Assert.Equal(1, new WizardPager(0).Count);
    }

    // Seventy words in two paragraphs is about five lines, the longer paragraph first, and each
    // paragraph ends on a short line.
    [Fact]
    public void ALetterDrawsItsWordsAtAboutFifteenToALine()
    {
        var lines = StyleReveal.LetterLines(words: 70, paragraphs: 2);
        Assert.Equal(new[] { 3, 2 }, lines.Select(paragraph => paragraph.Length).ToArray());
        foreach (var paragraph in lines)
        {
            Assert.True(paragraph[^1] < 0.8);
            Assert.All(paragraph[..^1], width => Assert.True(width > 0.9));
        }
    }

    [Fact]
    public void ALetterWithoutAParagraphCountDrawsTwo()
    {
        Assert.Equal(2, StyleReveal.LetterLines(words: 90, paragraphs: 0).Length);
        Assert.Equal(
            new[] { 2, 2 },
            StyleReveal.LetterLines(words: 0, paragraphs: 0).Select(paragraph => paragraph.Length).ToArray());
    }

    [Fact]
    public void ALetterHasALineForEveryParagraphAndRoomForAll()
    {
        Assert.Equal(
            new[] { 1, 1, 1 },
            StyleReveal.LetterLines(words: 10, paragraphs: 3).Select(paragraph => paragraph.Length).ToArray());
        Assert.Equal(12, StyleReveal.LetterLines(words: 900, paragraphs: 3).Sum(paragraph => paragraph.Length));
        Assert.Equal(6, StyleReveal.LetterLines(words: 200, paragraphs: 40).Length);
    }

    [Fact]
    public void ACardKeepsTheCoresHeadingOverTheWholeDescription()
    {
        var (headline, body) = StyleReveal.CardText(" Friendly and direct ", "On first-name terms. Brief.");
        Assert.Equal("Friendly and direct", headline);
        Assert.Equal("On first-name terms. Brief.", body);
    }

    [Fact]
    public void ACardWithoutAHeadingLeadsWithItsFirstSentenceOnce()
    {
        var card = StyleReveal.CardText("", "Thanks first, then the answer. Short lines.");
        Assert.Equal("Thanks first, then the answer", card.Headline);
        Assert.Equal("Short lines.", card.Body);
        var single = StyleReveal.CardText("", "Says no politely.");
        Assert.Equal("Says no politely", single.Headline);
        Assert.Empty(single.Body);
        Assert.Equal((string.Empty, string.Empty), StyleReveal.CardText("", "  "));
    }

    [Fact]
    public void APlaceholderIsARunOfItsOwnWithoutItsBrackets()
    {
        Assert.Equal(
            new[] { new RevealRun("Hi ", false), new RevealRun("Name", true), new RevealRun(",", false) },
            StyleReveal.Runs("Hi [Name],"));
        Assert.Equal(new[] { new RevealRun("Cheers,", false) }, StyleReveal.Runs("Cheers,"));
        Assert.Equal(new[] { new RevealRun("Name", true) }, StyleReveal.Runs("[Name]"));
        Assert.Equal(new[] { new RevealRun("Hi [] all", false) }, StyleReveal.Runs("Hi [] all"));
    }

    [Fact]
    public void SinceIsTheDayThisYearAndTheMonthBeforeIt()
    {
        var british = CultureInfo.GetCultureInfo("en-GB");
        // 2026-09-24 12:00 UTC.
        var now = DateTimeOffset.FromUnixTimeSeconds(1_790_251_200);
        // 2026-06-24 and 2025-03-03.
        Assert.Equal("24 June", StyleReveal.Since(1_782_302_400, now, TimeZoneInfo.Utc, british));
        Assert.Equal("Mar 2025", StyleReveal.Since(1_740_960_000, now, TimeZoneInfo.Utc, british));
    }

    // A style with no name is a row nobody can tell apart in the pickers; and only what changed is
    // written back.
    [Fact]
    public void SaveWaitsForANameAndWritesOnlyWhatChanged()
    {
        var sheet = new RevealSheet("s", "Work", "Short.", languages: 1);
        Assert.True(sheet.CanSave);
        Assert.Null(sheet.Rename("Work"));
        Assert.Null(sheet.NewNotes("Short."));
        sheet.Name = "  ";
        Assert.False(sheet.CanSave);
        sheet.Name = "  Home ";
        sheet.Notes = "Never sign with initials.";
        Assert.Equal("Home", sheet.Rename("Work"));
        Assert.Equal("Never sign with initials.", sheet.NewNotes("Short."));
    }
}
