// The grid chrome's own words, the half of CalendarFormat that reaches the resource catalogue.
//
// Split from CalendarFormat.cs so that file can stay pure BCL and be linked into Mailcal.Tests: L10n
// is generated over `Microsoft.Windows.ApplicationModel.Resources`, which a plain net10.0 assembly
// cannot reference, so one L10n call in a file is enough to put the whole file out of reach of the
// tests. Same seam as CalendarEventSummary (pure buckets) versus CalendarEventText (their words).
using System.Collections.Generic;
using System.Globalization;

namespace Allodia.Mailcal.Calendar;

/// <summary>
/// The chrome's own words, the ones that belong to the grid rather than to any week in it.
/// </summary>
/// <remarks>
/// Localised once, for the same reason as everything else in <see cref="CalendarFormat"/>: reading
/// them out of the resource catalogue inside a frame is a cost that buys nothing, since a zoom
/// cannot change them.
/// </remarks>
internal sealed record SurfaceStrings(
    string WeekShort,
    string AllDay,
    string Loading,
    string Now,
    /// <summary>The ruler's 24 labels. Midnight's is empty, see <see cref="CalendarFormat.HourLabel"/>.</summary>
    IReadOnlyList<string> Hours)
{
    internal static SurfaceStrings Of(bool use24Hour, CultureInfo culture)
    {
        var hours = new string[CalendarUnits.HoursInDay];
        for (var h = 0; h < CalendarUnits.HoursInDay; h++)
        {
            hours[h] = CalendarFormat.HourLabel(h, use24Hour, culture);
        }
        return new SurfaceStrings(
            L10n.CalendarWeekShort(),
            L10n.CalendarAllDay(),
            L10n.CalendarLoadingRange(),
            L10n.CalendarNow(),
            hours);
    }
}
