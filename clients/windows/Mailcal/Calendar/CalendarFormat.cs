// The grid's words and numbers, every one of them assembled client-side.
//
// The Rust core has no runtime locale facility at all (AGENTS.md: "Localisation is client-side"). It
// emits ISO dates and wall-clock minutes; the weekday headings, the hour ruler, the clock on a block
// and the period title in the header are all built here.
//
// All of it runs ONCE per page, never inside a frame (§7). Formatting a clock per block per frame of
// a pinch was measured on Android to cost far more than the arithmetic it sat next to.
//
// This file is PURE, BCL only, no L10n, no WinUI, so Mailcal.Tests can link it, and so the
// invitation card's preview reads its hours off the same function the grid's ruler does rather than
// growing a second clock. The half that *does* reach the resource catalogue (SurfaceStrings) is its
// own file, CalendarSurfaceStrings.cs, for exactly that reason: L10n.cs opens with
// `using Microsoft.Windows.ApplicationModel.Resources`, which a plain net10.0 assembly cannot have.
using System;
using System.Globalization;

namespace Allodia.Mailcal.Calendar;

/// <summary>The grid's localised copy.</summary>
internal static class CalendarFormat
{
    /// <summary>
    /// One hour-ruler label.
    /// </summary>
    /// <remarks>
    /// Midnight's is deliberately empty: it would collide with the day headings directly above, and
    /// the top gridline is unambiguous without it.
    /// </remarks>
    internal static string HourLabel(int hour, bool use24Hour, CultureInfo culture)
    {
        if (hour == 0)
        {
            return string.Empty;
        }
        return use24Hour
            ? hour.ToString("00", culture) + ":00"
            : new DateTime(2000, 1, 1, hour, 0, 0, DateTimeKind.Unspecified).ToString("h tt", culture);
    }

    /// <summary>A wall-clock time, from minutes past midnight.</summary>
    internal static string ClockTime(int minutes, bool use24Hour, CultureInfo culture)
    {
        var m = Math.Clamp(minutes, 0, (CalendarUnits.HoursInDay * 60) - 1);
        var time = new TimeOnly(m / 60, m % 60);
        return use24Hour
            ? time.ToString("HH:mm", culture)
            : time.ToString("h:mm tt", culture);
    }

    /// <summary>The clock a block carries: its start, and its end.</summary>
    internal static string TimeRange(uint startMinutes, uint endMinutes, bool use24Hour, CultureInfo culture) =>
        ClockTime((int)startMinutes, use24Hour, culture) +
        "–" +
        ClockTime((int)endMinutes, use24Hour, culture);

    /// <summary>
    /// The header's period title for the days on screen, "July 2026", or "Jun – Jul 2026" across a
    /// boundary.
    /// </summary>
    /// <remarks>
    /// Titled from the days actually <b>visible</b>, not from the page's whole week: at the day zoom
    /// the user is looking at one column, and naming the month of a Sunday they cannot see is a small
    /// lie that reads as a bug.
    /// </remarks>
    internal static string PeriodTitle(DateOnly first, DateOnly last, CultureInfo culture)
    {
        var months = culture.DateTimeFormat.AbbreviatedMonthNames;
        var firstMonth = months[first.Month - 1];
        if (first.Year == last.Year && first.Month == last.Month)
        {
            return $"{firstMonth} {first.Year.ToString(culture)}";
        }
        var lastMonth = months[last.Month - 1];
        return first.Year == last.Year
            ? $"{firstMonth} – {lastMonth} {last.Year.ToString(culture)}"
            : $"{firstMonth} {first.Year.ToString(culture)} – {lastMonth} {last.Year.ToString(culture)}";
    }
}
