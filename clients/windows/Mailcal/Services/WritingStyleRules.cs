// What Settings → Writing style does with the core's records (docs/ai.md rules 8 to 16): which
// outcome is said, once; a name's Save as drawn; the run's progress line; a range, a day and a
// language in words; and the language Learn is told the app is read in. Which stage is next, which
// control is held and which remedy a failure offers are all the core's. WinUI-free and L10n-free,
// so Mailcal.Tests can link it; the words arrive as parameters.

using System.Globalization;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The catalog's <c>a11y_letter_*</c> sentences, each given its value.</summary>
internal sealed record LetterWords(
    Func<string, string> Opening,
    Func<int, string> Words,
    Func<int, string> Paragraphs,
    Func<string, string> Closing,
    Func<string, string> Name);

/// <summary>The catalog's words these rules choose between.</summary>
internal sealed record WritingStyleWords(
    string RefusedBlankName,
    string RefusedGone,
    string ProgressStarting,
    Func<int, int, string> Progress,
    string RangeLastTwelveMonths,
    string RangeEverything,
    string RangeUntilEndOfLastYear,
    string RangeBetween,
    string FetchOlder,
    string FetchWholeHistory,
    string FrequencyMostly,
    string FrequencyOften,
    string FrequencySometimes);

internal static class WritingStyleRules
{
    /// <summary>Why a change to the library was refused, as the surface that sent it says it.</summary>
    internal static string Say(WritingStyleRefusal refusal, WritingStyleWords words) => refusal switch
    {
        WritingStyleRefusal.BlankName => words.RefusedBlankName,
        WritingStyleRefusal.Gone => words.RefusedGone,
        _ => throw new ArgumentOutOfRangeException(nameof(refusal), refusal, null),
    };

    /// <summary>A name's Save as drawn: the record's, and held besides while the core refuses the
    /// name typed (<c>writing_style_name_refusal</c>).</summary>
    internal static ControlState NameSave(ControlState save, WritingStyleRefusal? refusal) =>
        new(save.Enabled && refusal is null, save.Running);

    /// <summary>The run's line: done of total once the run knows how many requests it makes, and
    /// no fraction before.</summary>
    internal static string Progress(uint done, uint total, WritingStyleWords words) =>
        total == 0 ? words.ProgressStarting : words.Progress((int)done, (int)total);

    /// <summary>
    /// The miniature letter as it is spoken, one element whose grey lines are not read: how it
    /// opens, its length, its paragraphs, how it closes and the name, joined by a comma, each left
    /// out when it is empty or zero. A greeting or sign-off is its runs read as written, a
    /// placeholder as its word; the paragraphs are the record's count, never the lines drawn.
    /// </summary>
    internal static string LetterSpoken(WritingStyleLetter letter, LetterWords words)
    {
        var parts = new List<string>();
        var opening = string.Concat(letter.Opening.Select(run => run.Text));
        if (opening.Length > 0)
        {
            parts.Add(words.Opening(opening));
        }
        if (letter.Words > 0)
        {
            parts.Add(words.Words((int)letter.Words));
        }
        if (letter.ParagraphCount > 0)
        {
            parts.Add(words.Paragraphs((int)letter.ParagraphCount));
        }
        var closing = string.Concat(letter.Closing.Select(run => run.Text));
        if (closing.Length > 0)
        {
            parts.Add(words.Closing(closing));
        }
        if (letter.SignsAs.Length > 0)
        {
            parts.Add(words.Name(letter.SignsAs));
        }
        return string.Join(", ", parts);
    }

    /// <summary>A preset range's label. A range of the person's own is labelled by the choice it is,
    /// its days drawn by its own fields.</summary>
    internal static string Range(StyleLearnRange range, WritingStyleWords words) => range switch
    {
        StyleLearnRange.LastTwelveMonths => words.RangeLastTwelveMonths,
        StyleLearnRange.Everything => words.RangeEverything,
        StyleLearnRange.UntilEndOfLastYear => words.RangeUntilEndOfLastYear,
        StyleLearnRange.Between => words.RangeBetween,
        _ => throw new ArgumentOutOfRangeException(nameof(range), range, null),
    };

    /// <summary>The line under the ranges saying what Next fetches for the range chosen, or
    /// <c>null</c> when it fetches nothing.</summary>
    internal static string? Fetch(StyleRangeFetch fetch, WritingStyleWords words) => fetch switch
    {
        StyleRangeFetch.Older => words.FetchOlder,
        StyleRangeFetch.WholeHistory => words.FetchWholeHistory,
        _ => null,
    };

    /// <summary>How often a greeting or sign-off is used, in words.</summary>
    internal static string Frequency(WritingStyleFrequency frequency, WritingStyleWords words) => frequency switch
    {
        WritingStyleFrequency.Mostly => words.FrequencyMostly,
        WritingStyleFrequency.Often => words.FrequencyOften,
        _ => words.FrequencySometimes,
    };

    /// <summary>A record's <c>YYYY-MM-DD</c> day, or <c>null</c> when it is not one.</summary>
    internal static DateOnly? Day(string day) =>
        DateOnly.TryParseExact(day, "yyyy-MM-dd", CultureInfo.InvariantCulture, DateTimeStyles.None, out var parsed)
            ? parsed
            : null;

    /// <summary>A day for a <c>YYYY-MM-DD</c> field.</summary>
    internal static string Day(DateOnly day) => day.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);

    /// <summary>A record's moment, seconds since the Unix epoch, as the day it falls on in
    /// <paramref name="zone"/>, the display zone.</summary>
    internal static DateOnly DayOf(long seconds, TimeZoneInfo zone) =>
        DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(seconds), zone).DateTime);

    /// <summary>A day as the person reads one: its number, the month's name and the year, without
    /// the weekday, which says nothing here and makes a figure too wide to stand beside others.</summary>
    internal static string LongDate(DateOnly day, CultureInfo culture) => day.ToString("d MMMM yyyy", culture);

    /// <summary>
    /// A language's name, from its ISO 639-1 code: the catalog's own name for a language the app
    /// ships (<paramref name="catalogName"/>, which answers the key it was asked for when it has
    /// none), otherwise the culture's name for it, otherwise the code.
    /// </summary>
    internal static string LanguageName(string code, Func<string, string> catalogName)
    {
        var named = catalogName(code);
        if (named.Length > 0 && !named.StartsWith("settings_language_", StringComparison.Ordinal))
        {
            return named;
        }
        try
        {
            var language = CultureInfo.GetCultureInfo(code);
            return language.ThreeLetterISOLanguageName == "ivl" ? code : language.NativeName;
        }
        catch (CultureNotFoundException)
        {
            return code;
        }
    }

    /// <summary>
    /// The language Learn says the app is read in, an ISO 639-1 code: the language the person chose
    /// in Settings, otherwise the system's when the catalog ships it, otherwise English, which is
    /// what the catalog falls back to.
    /// </summary>
    internal static string AppLanguage(string stored, string system, IReadOnlyCollection<string> locales) =>
        locales.Contains(stored) ? stored : locales.Contains(system) ? system : "en";
}

/// <summary>
/// The library changes the page has already seen, from the moment Settings opened, so each newer
/// outcome is said once: a page drawn again does not say an old one twice.
/// </summary>
internal sealed class WritingStyleOutcomes
{
    private ulong _seen;

    internal WritingStyleOutcomes(WritingStylePage? page) => _seen = page?.Outcome?.Serial ?? 0;

    /// <summary>
    /// The outcome when it is one this page has not acted on, or <c>null</c>. A change that was
    /// made shows its result; one not written is the failed-save notice's; a refusal is said by
    /// the surface that sent it.
    /// </summary>
    internal WritingStyleOutcome? Fresh(WritingStyleOutcome? outcome)
    {
        if (outcome is null || outcome.Serial <= _seen)
        {
            return null;
        }
        _seen = outcome.Serial;
        return outcome;
    }
}
