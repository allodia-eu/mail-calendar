// When the time grid shows its loading pill, and where (docs/calendar.md, "Showing a period that is
// loading"). Free of WinUI so Mailcal.Tests can state the rule; the pill itself is
// CalendarLoadingPill.
using System;

namespace Allodia.Mailcal.Calendar;

/// <summary>Where the time grid's loading pill sits, in the surface's DIPs.</summary>
/// <param name="Shown">Whether a week on screen is loading.</param>
/// <param name="Left">The left edge of the days the pill is centred over: the ruler's width.</param>
/// <param name="Top">The pill's top: just below the day headings and the all-day banner.</param>
internal readonly record struct LoadingPillPlace(bool Shown, float Left, float Top);

internal static class CalendarLoading
{
    /// <summary>The gap between the top of the hours and the pill.</summary>
    internal const float PillGap = 12f;

    /// <summary>
    /// The pill for the days <paramref name="firstDay"/> to <paramref name="lastDay"/> on screen,
    /// counted from the anchor week's first day.
    /// </summary>
    /// <remarks>
    /// Shown while <b>any</b> week those days fall in is loading: at a seam the days on screen belong
    /// to two weeks, and the one not yet materialized is still on screen. It sits at the top of the
    /// hours, not of the day, so it is seen at whatever hour the day opened.
    /// </remarks>
    internal static LoadingPillPlace Place(
        int firstDay,
        int lastDay,
        Func<int, bool> weekLoading,
        SurfaceMetrics m)
    {
        var shown = false;
        for (var week = WeekOf(firstDay); week <= WeekOf(lastDay) && !shown; week++)
        {
            shown = weekLoading(week);
        }
        return new LoadingPillPlace(shown, m.Gutter, m.ContentTop + PillGap);
    }

    private static int WeekOf(int day) => (int)Math.Floor(day / (double)CalendarUnits.DaysInWeek);
}
