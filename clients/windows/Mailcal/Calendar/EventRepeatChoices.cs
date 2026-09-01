// What the repeat editor's controls offer, and the arithmetic behind the weekday row.
//
// WinUI-free AND L10n-free on purpose: the words are EventRepeatEditor (Dialogs/), the same seam
// EventRepeatFormat / EventRepeatText uses, so the decisions stay reachable from Mailcal.Tests.
//
// Two of them are silent when wrong. DayOfWeek counts Sunday as 0 while the core counts from
// Monday, so an off-by-one renames every day of the week and still draws a plausible row; and a
// weekly rule left with no day ticked is one the core refuses, which reads in the app as a save
// that simply did nothing.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>What the frequency picker offers, including the choice not to repeat.</summary>
internal enum RepeatChoice
{
    Never,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// <summary>What the "Ends" picker offers.</summary>
internal enum RepeatEndChoice
{
    Never,
    OnDate,
    AfterCount,
}

internal static class EventRepeatChoices
{
    /// <summary>
    /// The most periods, and the most instances, either box will go to. Well under the core's own
    /// ceiling, which refuses a rule no calendar could draw.
    /// </summary>
    internal const int Ceiling = 999;

    /// <summary>The week, Monday first: the order the core counts weekdays in.</summary>
    private static readonly RecurrenceWeekday[] Week =
    [
        RecurrenceWeekday.Monday,
        RecurrenceWeekday.Tuesday,
        RecurrenceWeekday.Wednesday,
        RecurrenceWeekday.Thursday,
        RecurrenceWeekday.Friday,
        RecurrenceWeekday.Saturday,
        RecurrenceWeekday.Sunday,
    ];

    internal static readonly RepeatChoice[] All = Enum.GetValues<RepeatChoice>();

    internal static readonly RepeatEndChoice[] AllEnds = Enum.GetValues<RepeatEndChoice>();

    internal static RepeatChoice ChoiceOf(RecurrenceFrequency? frequency) => frequency switch
    {
        null => RepeatChoice.Never,
        RecurrenceFrequency.Daily => RepeatChoice.Daily,
        RecurrenceFrequency.Weekly => RepeatChoice.Weekly,
        RecurrenceFrequency.Monthly => RepeatChoice.Monthly,
        _ => RepeatChoice.Yearly,
    };

    /// <summary>The frequency a choice stands for. Never called for <see cref="RepeatChoice.Never"/>,
    /// which is the absence of a rule rather than a frequency.</summary>
    internal static RecurrenceFrequency Frequency(RepeatChoice choice) => choice switch
    {
        RepeatChoice.Daily => RecurrenceFrequency.Daily,
        RepeatChoice.Weekly => RecurrenceFrequency.Weekly,
        RepeatChoice.Monthly => RecurrenceFrequency.Monthly,
        _ => RecurrenceFrequency.Yearly,
    };

    internal static RepeatEndChoice EndChoiceOf(RecurrenceEnd end) => end switch
    {
        RecurrenceEnd.OnDate => RepeatEndChoice.OnDate,
        RecurrenceEnd.AfterCount => RepeatEndChoice.AfterCount,
        _ => RepeatEndChoice.Never,
    };

    /// <summary>The weekdays in the order this machine's culture starts its week on.</summary>
    internal static IReadOnlyList<RecurrenceWeekday> LocalWeekOrder(CultureInfo culture)
    {
        var first = ((int)culture.DateTimeFormat.FirstDayOfWeek + 6) % 7;
        return [.. Week.Skip(first), .. Week.Take(first)];
    }

    /// <summary>The weekday a rule first chosen on this event should fall on.</summary>
    internal static RecurrenceWeekday WeekdayOf(DateTime date) => Week[((int)date.DayOfWeek + 6) % 7];

    /// <summary>The platform's own day-of-week for one of the core's, for its culture data.</summary>
    internal static DayOfWeek DayOf(RecurrenceWeekday day) => day switch
    {
        RecurrenceWeekday.Monday => DayOfWeek.Monday,
        RecurrenceWeekday.Tuesday => DayOfWeek.Tuesday,
        RecurrenceWeekday.Wednesday => DayOfWeek.Wednesday,
        RecurrenceWeekday.Thursday => DayOfWeek.Thursday,
        RecurrenceWeekday.Friday => DayOfWeek.Friday,
        RecurrenceWeekday.Saturday => DayOfWeek.Saturday,
        _ => DayOfWeek.Sunday,
    };

    /// <summary>
    /// Ticks or unticks one weekday, returning the row in week order. At least one day stays
    /// ticked: a weekly rule that names none is not a rule, and the core would refuse it.
    /// </summary>
    internal static RecurrenceWeekday[] Toggled(
        RecurrenceWeekday[] current,
        RecurrenceWeekday day,
        IReadOnlyList<RecurrenceWeekday> order)
    {
        RecurrenceWeekday[] next;
        if (!current.Contains(day))
        {
            next = [.. current, day];
        }
        else if (current.Length > 1)
        {
            next = [.. current.Where(d => d != day)];
        }
        else
        {
            return current;
        }
        return [.. order.Where(next.Contains)];
    }

    /// <summary>The wall clock a repeat's end date is written as: a date, at midnight.</summary>
    internal static string EndDateWallClock(DateTime date) =>
        date.ToString("yyyy-MM-dd'T'00:00:00", CultureInfo.InvariantCulture);

    /// <summary>The date part of an end wall clock, or <paramref name="fallback"/> if unreadable.</summary>
    internal static DateTime EndDateOf(string wallClock, DateTime fallback) =>
        DateTime.TryParse(
            wallClock.Split('T')[0], CultureInfo.InvariantCulture, DateTimeStyles.None, out var date)
            ? date
            : fallback;
}
