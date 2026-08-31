// The repeat rule as a sentence: "Every 2 weeks on Monday, Friday, until 3 Jun 2027".
//
// Wording only. Which frame states the rule, and the weekday and month names that go in it, are
// decided in EventRepeatFormat (Calendar/), where Mailcal.Tests can reach them; this maps a frame
// to its catalog string, which is a WinUI resource call and so cannot live in the test-linked half.
// The same seam CalendarEventText uses for reminders, and InvitationText for the invitation card.
using System.Globalization;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The repeat summary the event detail and the editor both read from.</summary>
internal static class EventRepeatText
{
    /// <summary>
    /// The repeat summary, localised.
    /// </summary>
    /// <param name="summary">
    /// The rule as a sentence's parts. <c>null</c> for an event with no rule, and for one whose
    /// rule the core would not state exactly, those get the bare <i>Repeats</i>, because
    /// approximating states a series the user does not have and nothing on screen would tell them
    /// apart.
    /// </param>
    /// <param name="isRecurring">Whether the event repeats at all.</param>
    /// <param name="culture">Whose weekday names, month names and dates to write it in.</param>
    internal static string Summary(RepeatSummary? summary, bool isRecurring, CultureInfo culture)
    {
        if (summary is null)
        {
            return isRecurring ? L10n.EventRepeatOther() : L10n.EventRepeatNone();
        }
        var rule = Rhythm(
            EventRepeatFormat.PhraseOf(summary.Rhythm, culture, L10n.EventRepeatNthAltDays()));
        return summary.Stop switch
        {
            RepeatStop.OnDate stop =>
                L10n.EventRepeatSumUntil(rule, EventRepeatFormat.EndDate(stop.Date, culture)),
            RepeatStop.AfterCount stop => L10n.EventRepeatSumTimes(rule, (int)stop.Count),
            _ => rule,
        };
    }

    /// <summary>The rhythm alone, without what ends it.</summary>
    private static string Rhythm(RepeatPhrase phrase) => phrase.Frame switch
    {
        RepeatFrame.Daily => L10n.EventRepeatDaily(),
        RepeatFrame.DailyEvery => L10n.EventRepeatSumDailyN(phrase.Interval),
        RepeatFrame.Weekly => L10n.EventRepeatSumWeekly(phrase.Days),
        RepeatFrame.WeeklyEvery => L10n.EventRepeatSumWeeklyN(phrase.Interval, phrase.Days),
        RepeatFrame.MonthlyOnDay => L10n.EventRepeatSumMonthlyDay(phrase.Day),
        RepeatFrame.MonthlyOnDayEvery =>
            L10n.EventRepeatSumMonthlyDayN(phrase.Interval, phrase.Day),
        RepeatFrame.MonthlyOnLastDay => L10n.EventRepeatSumMonthlyLast(),
        RepeatFrame.MonthlyOnLastDayEvery => L10n.EventRepeatSumMonthlyLastN(phrase.Interval),
        RepeatFrame.MonthlyOnWeekday => L10n.EventRepeatSumMonthlyNth(Position(phrase)),
        RepeatFrame.MonthlyOnWeekdayEvery =>
            L10n.EventRepeatSumMonthlyNthN(phrase.Interval, Position(phrase)),
        RepeatFrame.YearlyOnDate => L10n.EventRepeatSumYearly(phrase.Day, phrase.Month),
        RepeatFrame.YearlyOnDateEvery =>
            L10n.EventRepeatSumYearlyN(phrase.Interval, phrase.Day, phrase.Month),
        RepeatFrame.YearlyOnWeekday =>
            L10n.EventRepeatSumYearlyNth(Position(phrase), phrase.Month),
        _ => L10n.EventRepeatSumYearlyNthN(phrase.Interval, Position(phrase), phrase.Month),
    };

    /// <summary>
    /// "on the fourth Monday", the phrase both by-weekday frames drop into, in the wording the
    /// weekday agrees with. Which wording that is was decided in
    /// <see cref="RepeatPosition"/>; here it only picks the string.
    /// </summary>
    private static string Position(RepeatPhrase phrase)
    {
        // Only the by-weekday frames reach this, and PhraseOf fills the position for every one.
        var position = phrase.Position!;
        var weekday = position.Weekday;
        return position.Ordinal switch
        {
            RepeatOrdinal.First => position.Alternative
                ? L10n.EventRepeatNthFirstAlt(weekday)
                : L10n.EventRepeatNthFirst(weekday),
            RepeatOrdinal.Second => position.Alternative
                ? L10n.EventRepeatNthSecondAlt(weekday)
                : L10n.EventRepeatNthSecond(weekday),
            RepeatOrdinal.Third => position.Alternative
                ? L10n.EventRepeatNthThirdAlt(weekday)
                : L10n.EventRepeatNthThird(weekday),
            RepeatOrdinal.Fourth => position.Alternative
                ? L10n.EventRepeatNthFourthAlt(weekday)
                : L10n.EventRepeatNthFourth(weekday),
            RepeatOrdinal.Fifth => position.Alternative
                ? L10n.EventRepeatNthFifthAlt(weekday)
                : L10n.EventRepeatNthFifth(weekday),
            _ => position.Alternative
                ? L10n.EventRepeatNthLastAlt(weekday)
                : L10n.EventRepeatNthLast(weekday),
        };
    }
}
