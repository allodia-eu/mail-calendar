// The calendar's focus date on Windows (docs/calendar.md, "The focus date"): the one day every view
// is drawn around. The core holds it and decides every step; this keeps the client's copy and moves
// it only through the core, so the two cannot disagree.
using System;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>The core's focus date (<c>set_calendar_focus</c>, <c>step_calendar_focus</c>), behind a
/// seam a test can fake.</summary>
internal interface ICalendarFocusCore
{
    void Set(DateOnly date);

    /// <summary>Moves the focus <paramref name="steps"/> periods of <paramref name="layout"/> and
    /// returns where it landed.</summary>
    DateOnly Step(CalendarLayout layout, int steps, DateOnly today);
}

/// <summary>The focus date and the three things that move it: a press, a step, and the strip coming to
/// rest somewhere a gesture took it.</summary>
internal sealed class CalendarFocus(ICalendarFocusCore core, DateOnly today)
{
    /// <summary>The day every view is drawn around.</summary>
    internal DateOnly Focus { get; private set; } = today;

    /// <summary>Puts the focus on <paramref name="date"/>: a day pressed, Today, a launch.</summary>
    internal void Set(DateOnly date)
    {
        core.Set(date);
        Focus = date;
    }

    /// <summary>Previous or next in <paramref name="mode"/>: the core moves the focus one period of it
    /// per step, and lands a step into the week or month holding today on today.</summary>
    internal DateOnly Step(CalendarMode mode, int steps, DateOnly today)
    {
        Focus = core.Step(mode.ToLayout(), steps, today);
        return Focus;
    }

    /// <summary>
    /// The time grid came to rest with <paramref name="count"/> days from <paramref name="first"/> on
    /// screen, wherever a swipe, a wheel or a step took it.
    /// </summary>
    /// <remarks>
    /// A step has already moved the focus to where the strip lands, so this changes nothing after one.
    /// After a gesture the focus follows the days on screen. In a week or a work week it moves by whole
    /// weeks to the week in the middle of the screen, through the core, so it keeps its weekday and a
    /// landing in today's week lands on today; a weekend day the work week does not draw is still in
    /// that week, so a switch to the work week cannot move it. A day and three days are anchored on the
    /// focus itself, so it moves to the first day shown.
    /// </remarks>
    internal void Rest(CalendarMode mode, DateOnly first, int count, DateOnly today, Func<DateOnly, DateOnly> weekStart)
    {
        if (count <= 0 || !mode.IsGrid())
        {
            return;
        }
        var last = first.AddDays(count - 1);
        if (mode is CalendarMode.Week or CalendarMode.WorkWeek)
        {
            var week = weekStart(Focus);
            if (week >= weekStart(first) && week <= weekStart(last))
            {
                return;
            }
            var middle = weekStart(first.AddDays(count / 2));
            Step(CalendarMode.Week, (middle.DayNumber - week.DayNumber) / CalendarUnits.DaysInWeek, today);
            return;
        }
        if (Focus < first || Focus > last)
        {
            Set(first);
        }
    }
}
