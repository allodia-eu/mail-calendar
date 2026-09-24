// The reveal's steps and the arithmetic behind its pictures (docs/ai.md, "Learning" step 5): which
// pages are about one language, how many grey lines the miniature letter draws, what a card says
// when the core gave it no heading, a greeting's placeholder apart from its words, the "Since"
// figure, and when the name and notes may be saved. WinUI-free and L10n-free, so Mailcal.Tests can
// pin each rule; each is one a page gets wrong while looking right.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;

namespace Allodia.Mailcal.Services;

/// <summary>The reveal's pages, in the order it walks them.</summary>
internal enum RevealStep
{
    /// <summary>How many messages, in which languages, since when, from which account.</summary>
    Read,

    /// <summary>A typical reply, as a miniature letter.</summary>
    Letter,

    /// <summary>Greetings and sign-offs.</summary>
    Habits,

    /// <summary>Tone and approach, one card each.</summary>
    Voice,

    /// <summary>The person's phrases, and what they avoid.</summary>
    Phrases,

    /// <summary>The style's name and the person's own notes.</summary>
    Name,
}

/// <summary>
/// One run of a greeting or sign-off: words, or a placeholder such as <c>[Name]</c> without its
/// brackets, which the reveal draws as a pill because there is no name to put in its place.
/// </summary>
/// <param name="Text">The words, or the placeholder's inside.</param>
/// <param name="Placeholder">Whether this run is a placeholder.</param>
internal sealed record RevealRun(string Text, bool Placeholder);

/// <summary>The reveal's rules.</summary>
internal static class StyleReveal
{
    private static readonly double[] FullLine = [1, 0.96, 0.98];
    private static readonly double[] LastLine = [0.58, 0.74, 0.44, 0.66];

    /// <summary>The pages, in order.</summary>
    internal static IReadOnlyList<RevealStep> Steps { get; } = Enum.GetValues<RevealStep>();

    /// <summary>Whether <paramref name="step"/> is about one language, which is what puts the
    /// language control on screen.</summary>
    internal static bool IsPerLanguage(this RevealStep step) =>
        step is RevealStep.Letter or RevealStep.Habits or RevealStep.Voice or RevealStep.Phrases;

    /// <summary>Whether there is a language to choose: a single one has nothing to choose between.</summary>
    internal static bool ChoosesLanguage(int languages) => languages > 1;

    /// <summary>
    /// The miniature letter's grey lines: one list per paragraph, each line's width as a share of
    /// the letter's. About fifteen words to a line, two paragraphs when the style does not say, and
    /// never more lines than the letter has room for, so a long typical reply still reads as a
    /// letter. Each paragraph ends on a short line.
    /// </summary>
    internal static double[][] LetterLines(uint words, uint paragraphs)
    {
        var count = paragraphs == 0 ? 2 : (int)Math.Min(paragraphs, 6u);
        var wanted = words == 0 ? count * 2 : (int)Math.Round(words / 15.0, MidpointRounding.AwayFromZero);
        var lines = Math.Min(Math.Max(wanted, count), 12);
        return Enumerable.Range(0, count)
            .Select(paragraph =>
            {
                var length = (lines / count) + (paragraph < lines % count ? 1 : 0);
                return Enumerable.Range(0, length)
                    .Select(line => line == length - 1
                        ? LastLine[paragraph % LastLine.Length]
                        : FullLine[line % FullLine.Length])
                    .ToArray();
            })
            .ToArray();
    }

    /// <summary>
    /// A card's heading and the text beneath it. The core's heading goes over the whole
    /// description; without one, the description's first sentence becomes the heading and the rest
    /// stays beneath, so no sentence is shown twice.
    /// </summary>
    internal static (string Headline, string Body) CardText(string headline, string description)
    {
        var heading = headline.Trim();
        var text = description.Trim();
        if (heading.Length > 0)
        {
            return (heading, text);
        }
        if (text.Length == 0)
        {
            return (string.Empty, string.Empty);
        }
        var end = SentenceEnd(text);
        var sentence = text[..end].Trim();
        if (sentence.EndsWith('.'))
        {
            sentence = sentence[..^1];
        }
        return (sentence, text[end..].Trim());
    }

    /// <summary><paramref name="text"/> cut into runs, each placeholder apart from the words around
    /// it and without its brackets. Empty brackets are words.</summary>
    internal static IReadOnlyList<RevealRun> Runs(string text)
    {
        var runs = new List<RevealRun>();
        var start = 0;
        var at = 0;
        while (at < text.Length)
        {
            var close = text[at] == '[' ? text.IndexOfAny(['[', ']'], at + 1) : -1;
            if (close > at + 1 && text[close] == ']')
            {
                if (at > start)
                {
                    runs.Add(new RevealRun(text[start..at], false));
                }
                runs.Add(new RevealRun(text[(at + 1)..close], true));
                at = close + 1;
                start = at;
                continue;
            }
            at++;
        }
        if (start < text.Length)
        {
            runs.Add(new RevealRun(text[start..], false));
        }
        return runs;
    }

    /// <summary>
    /// The "Since" figure: the day and month within this year, the month and year before it,
    /// because it is drawn at the size of the counts beside it and a full date does not fit there.
    /// The order is <paramref name="culture"/>'s own.
    /// </summary>
    internal static string Since(long unixSeconds, DateTimeOffset now, TimeZoneInfo zone, CultureInfo culture)
    {
        var date = TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(unixSeconds), zone);
        var today = TimeZoneInfo.ConvertTime(now, zone);
        var format = culture.DateTimeFormat;
        return date.Year == today.Year
            ? date.ToString(format.MonthDayPattern, culture)
            : date.ToString(format.YearMonthPattern.Replace("MMMM", "MMM", StringComparison.Ordinal), culture);
    }

    // Where the first sentence ends: after the first full stop, question or exclamation mark that
    // closes the text or is followed by a space.
    private static int SentenceEnd(string text)
    {
        for (var at = 0; at < text.Length; at++)
        {
            if ((text[at] is '.' or '?' or '!') && (at + 1 == text.Length || char.IsWhiteSpace(text[at + 1])))
            {
                return at + 1;
            }
        }
        return text.Length;
    }
}

/// <summary>
/// One reveal on screen: its page, the language the per-language pages show, and the name and notes
/// as typed, which outlive a move between pages.
/// </summary>
internal sealed class RevealSheet
{
    private int _language;

    /// <summary>Opens the reveal of <paramref name="styleId"/> on its first page.</summary>
    internal RevealSheet(string styleId, string name, string notes, int languages)
    {
        StyleId = styleId;
        Name = name;
        Notes = notes;
        Languages = languages;
    }

    /// <summary>The style shown.</summary>
    internal string StyleId { get; }

    /// <summary>How many languages the style covers.</summary>
    internal int Languages { get; }

    /// <summary>The page on screen.</summary>
    internal WizardPager Pager { get; } = new(StyleReveal.Steps.Count);

    /// <summary>The step on screen.</summary>
    internal RevealStep Step => StyleReveal.Steps[Pager.Index];

    /// <summary>Which language the per-language pages show, held to the languages there are.</summary>
    internal int Language
    {
        get => _language;
        set => _language = Math.Clamp(value, 0, Math.Max(Languages - 1, 0));
    }

    /// <summary>Whether the language control is on screen: on a page about one language, when
    /// there is more than one.</summary>
    internal bool ShowsLanguageChoice => StyleReveal.ChoosesLanguage(Languages) && Step.IsPerLanguage();

    /// <summary>The name as typed.</summary>
    internal string Name { get; set; }

    /// <summary>The notes as typed.</summary>
    internal string Notes { get; set; }

    /// <summary>A style with no name is a row nobody can tell apart in the pickers, so Save waits
    /// for one.</summary>
    internal bool CanSave => !string.IsNullOrWhiteSpace(Name);

    /// <summary>The name to store, trimmed, or <c>null</c> when it is <paramref name="stored"/>.</summary>
    internal string? Rename(string stored)
    {
        var name = Name.Trim();
        return name.Length == 0 || name == stored ? null : name;
    }

    /// <summary>The notes to store, or <c>null</c> when they are <paramref name="stored"/>.</summary>
    internal string? NewNotes(string stored) => Notes == stored ? null : Notes;
}
