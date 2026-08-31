// The repeat rule reduced to the parts a sentence needs: which catalog frame states it, and the
// values that drop into that frame. The WinUI twin of Apple's EventRepeatText.swift and Android's
// EventRepeatText.kt, split across two files rather than their one, because L10n.cs cannot be
// linked into Mailcal.Tests, so a rule phrased as a string is a rule no test can reach. This half
// decides; EventRepeatText (Dialogs/) says it, the seam InvitationFormat / InvitationText uses.
//
// The core decided which sentence a rule gets, it read the event's start for every part the rule
// leaves out, put the weekdays in week order, and dropped the rules it cannot state exactly, so
// what is left here is choosing a frame over its every-N-periods twin, and naming the weekdays and
// months from the platform's own locale data, the way CalendarFormat names the grid's headings.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>
/// Which catalog frame states a rhythm. Every rhythm has two: one for a rule that repeats every
/// period, one for a rule that skips some, "Weekly" for a fortnightly meeting is a claim rather
/// than a summary, and the interval is the reason the core sends a structure rather than a word.
/// </summary>
internal enum RepeatFrame
{
    /// <summary>Every day.</summary>
    Daily,

    /// <summary>Every N days.</summary>
    DailyEvery,

    /// <summary>Weekly, on named weekdays.</summary>
    Weekly,

    /// <summary>Every N weeks, on named weekdays.</summary>
    WeeklyEvery,

    /// <summary>Monthly, on a day counted from the month's start.</summary>
    MonthlyOnDay,

    /// <summary>Every N months, on a day counted from the month's start.</summary>
    MonthlyOnDayEvery,

    /// <summary>Monthly, on the month's last day.</summary>
    MonthlyOnLastDay,

    /// <summary>Every N months, on the month's last day.</summary>
    MonthlyOnLastDayEvery,

    /// <summary>Monthly, on a weekday's position in the month.</summary>
    MonthlyOnWeekday,

    /// <summary>Every N months, on a weekday's position in the month.</summary>
    MonthlyOnWeekdayEvery,

    /// <summary>Annually, on one date.</summary>
    YearlyOnDate,

    /// <summary>Every N years, on one date.</summary>
    YearlyOnDateEvery,

    /// <summary>Annually, on a weekday's position inside one month.</summary>
    YearlyOnWeekday,

    /// <summary>Every N years, on a weekday's position inside one month.</summary>
    YearlyOnWeekdayEvery,
}

/// <summary>Which of a weekday's places in the month a rule counts to.</summary>
internal enum RepeatOrdinal
{
    /// <summary>The first one.</summary>
    First,

    /// <summary>The second one.</summary>
    Second,

    /// <summary>The third one.</summary>
    Third,

    /// <summary>The fourth one.</summary>
    Fourth,

    /// <summary>The fifth one.</summary>
    Fifth,

    /// <summary>The last one, whichever that turns out to be.</summary>
    Last,
}

/// <summary>
/// "on the fourth Monday", "na quarta segunda-feira", the phrase both by-weekday frames drop
/// into, <b>carrying its own article</b>.
/// </summary>
/// <param name="Ordinal">Which place in the month is counted to.</param>
/// <param name="Alternative">
/// Whether this weekday takes the ordinal's second wording. The article agrees with the weekday in
/// some languages, Italian inflects for <i>domenica</i>, Portuguese for <i>segunda</i> through
/// <i>sexta</i>, and the weekday is not known until here, so each position ships two wordings and
/// <b>which weekdays take the alternative one is stated in the catalog</b>
/// (<c>event_repeat_nth_alt_days</c>, ISO weekday numbers) rather than as a table of genders in
/// here: it is a fact about a language, and it belongs beside that language's words. A language
/// where the question does not arise leaves the set empty and ships the same wording twice.
/// </param>
/// <param name="Weekday">The weekday's name, in the reader's language.</param>
internal sealed record RepeatPosition(RepeatOrdinal Ordinal, bool Alternative, string Weekday);

/// <summary>
/// A rhythm as the frame that states it plus the values that go in it. Only the values that frame
/// takes are filled; the rest stay empty, because no frame reads them.
/// </summary>
/// <param name="Frame">The frame to state this rhythm with.</param>
/// <param name="Interval">Periods between instances; <c>1</c> is every one.</param>
/// <param name="Days">The weekdays, named and joined, for the weekly frames.</param>
/// <param name="Day">The day of the month, for the by-date frames.</param>
/// <param name="Month">The month's name, for the yearly frames.</param>
/// <param name="Position">The ordinal phrase, for the by-weekday frames.</param>
internal sealed record RepeatPhrase(
    RepeatFrame Frame,
    int Interval,
    string Days = "",
    string Day = "",
    string Month = "",
    RepeatPosition? Position = null);

/// <summary>The repeat summary's decisions, with no WinUI and no catalog string in them.</summary>
internal static class EventRepeatFormat
{
    /// <summary>The rhythm as the frame that states it and the values that go in that frame.</summary>
    /// <param name="rhythm">The rhythm the core decided on.</param>
    /// <param name="culture">Whose weekday and month names to use.</param>
    /// <param name="alternativeDays">
    /// The catalog's <c>event_repeat_nth_alt_days</c> entry, passed in so this stays reachable from
    /// a test assembly that cannot link L10n. See <see cref="RepeatPosition"/>.
    /// </param>
    internal static RepeatPhrase PhraseOf(
        RepeatRhythm rhythm,
        CultureInfo culture,
        string alternativeDays) => rhythm switch
        {
            RepeatRhythm.Daily r => new RepeatPhrase(
                Every(r.Interval, RepeatFrame.Daily, RepeatFrame.DailyEvery),
                (int)r.Interval),

            RepeatRhythm.Weekly r => new RepeatPhrase(
                Every(r.Interval, RepeatFrame.Weekly, RepeatFrame.WeeklyEvery),
                (int)r.Interval,
                Days: string.Join(", ", r.Days.Select(day => WeekdayName(day, culture)))),

            RepeatRhythm.MonthlyOnDay r => new RepeatPhrase(
                Every(r.Interval, RepeatFrame.MonthlyOnDay, RepeatFrame.MonthlyOnDayEvery),
                (int)r.Interval,
                Day: Number(r.Day)),

            RepeatRhythm.MonthlyOnLastDay r => new RepeatPhrase(
                Every(r.Interval, RepeatFrame.MonthlyOnLastDay, RepeatFrame.MonthlyOnLastDayEvery),
                (int)r.Interval),

            RepeatRhythm.MonthlyOnWeekday r => new RepeatPhrase(
                Every(r.Interval, RepeatFrame.MonthlyOnWeekday, RepeatFrame.MonthlyOnWeekdayEvery),
                (int)r.Interval,
                Position: PositionOf(r.Nth, r.Day, culture, alternativeDays)),

            RepeatRhythm.YearlyOnDate r => new RepeatPhrase(
                Every(r.Interval, RepeatFrame.YearlyOnDate, RepeatFrame.YearlyOnDateEvery),
                (int)r.Interval,
                Day: Number(r.Day),
                Month: MonthName(r.Month, culture)),

            RepeatRhythm.YearlyOnWeekday r => new RepeatPhrase(
                Every(r.Interval, RepeatFrame.YearlyOnWeekday, RepeatFrame.YearlyOnWeekdayEvery),
                (int)r.Interval,
                Month: MonthName(r.Month, culture),
                Position: PositionOf(r.Nth, r.Day, culture, alternativeDays)),

            _ => throw new ArgumentOutOfRangeException(nameof(rhythm)),
        };

    /// <summary>
    /// The last date a repeat may start on, written the way this client writes every other absolute
    /// date (docs/timestamps.md). Deliberately not the long form the detail's own date line uses:
    /// that one names the weekday, which inside a sentence about weekdays reads as one of them.
    /// </summary>
    internal static string EndDate(string iso, CultureInfo culture) =>
        DateTime.TryParseExact(
            iso,
            "yyyy-MM-dd",
            CultureInfo.InvariantCulture,
            DateTimeStyles.None,
            out var date)
            ? date.ToString("d MMM yyyy", culture)
            : iso;

    /// <summary>Which place in the month <paramref name="nth"/> counts to; <c>-1</c> is the last.</summary>
    internal static RepeatOrdinal OrdinalOf(int nth) => nth switch
    {
        1 => RepeatOrdinal.First,
        2 => RepeatOrdinal.Second,
        3 => RepeatOrdinal.Third,
        4 => RepeatOrdinal.Fourth,
        5 => RepeatOrdinal.Fifth,
        _ => RepeatOrdinal.Last,
    };

    /// <summary>
    /// The catalog's alternative-form weekdays, as ISO numbers. Empty for a language whose ordinal
    /// does not inflect, which is why an unparseable entry is simply dropped: the two wordings are
    /// the same string there, so nothing on screen can go wrong.
    /// </summary>
    internal static IReadOnlySet<int> AlternativeWeekdays(string catalogEntry) =>
        catalogEntry
            .Split(',')
            .Select(part => int.TryParse(
                part.Trim(),
                NumberStyles.Integer,
                CultureInfo.InvariantCulture,
                out var iso) ? iso : (int?)null)
            .OfType<int>()
            .ToHashSet();

    /// <summary>
    /// The core's weekday as its ISO number, Monday 1 through Sunday 7, which is what the catalog's
    /// alternative-form sets are written in.
    /// </summary>
    internal static int IsoWeekday(RecurrenceWeekday day) => day switch
    {
        RecurrenceWeekday.Monday => 1,
        RecurrenceWeekday.Tuesday => 2,
        RecurrenceWeekday.Wednesday => 3,
        RecurrenceWeekday.Thursday => 4,
        RecurrenceWeekday.Friday => 5,
        RecurrenceWeekday.Saturday => 6,
        _ => 7,
    };

    /// <summary>The weekday's name in <paramref name="culture"/>, the platform's word, not ours.</summary>
    internal static string WeekdayName(RecurrenceWeekday day, CultureInfo culture) =>
        // DayNames is indexed from Sunday, so an ISO number maps in by taking Sunday's 7 to 0.
        culture.DateTimeFormat.DayNames[IsoWeekday(day) % 7];

    /// <summary>The month's name in <paramref name="culture"/>, from a 1-based month number.</summary>
    internal static string MonthName(uint month, CultureInfo culture) =>
        culture.DateTimeFormat.MonthNames[(int)month - 1];

    private static RepeatPosition PositionOf(
        int nth,
        RecurrenceWeekday day,
        CultureInfo culture,
        string alternativeDays) => new(
            OrdinalOf(nth),
            AlternativeWeekdays(alternativeDays).Contains(IsoWeekday(day)),
            WeekdayName(day, culture));

    /// <summary>The frame for a rule that repeats every period, else the one that says how many.</summary>
    private static RepeatFrame Every(uint interval, RepeatFrame everyPeriod, RepeatFrame skipping) =>
        interval == 1 ? everyPeriod : skipping;

    /// <summary>A day-of-month as the frames take it, plain digits, as on the other clients.</summary>
    private static string Number(uint value) => value.ToString(CultureInfo.InvariantCulture);
}
