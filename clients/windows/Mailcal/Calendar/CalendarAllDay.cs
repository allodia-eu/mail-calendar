// How many all-day bars the banner shows before it starts hiding them, and how many it hid.
//
// The core stacks all-day and multi-day events into non-colliding **lanes** (in Rust, so no two
// clients can lane them differently) and reports the true lane count. It does NOT cap them: an
// uncapped banner grows a row per lane and, with a busy week, eats the grid it sits above.
//
// The cap is a client decision, it is a question of how much vertical room this screen has, the
// same category as the hour height, not a user setting the core must own. But the *rule* is shared,
// and it is written down in docs/calendar.md §4. This is Windows keeping it.
using System.Collections.Generic;

namespace Allodia.Mailcal.Calendar;

/// <summary>The all-day banner's cap, and what it hides.</summary>
internal static class CalendarAllDay
{
    /// <summary>
    /// How many rows the banner shows when collapsed.
    /// </summary>
    /// <remarks>
    /// With more lanes than this, the <b>last</b> visible row is given over to a per-day "+N" chip
    /// rather than to a bar, so the banner is never taller than this, and no event is silently
    /// dropped.
    /// </remarks>
    internal const int CollapsedLanes = 3;

    /// <summary>The lanes that still hold real bars once a "+N" row is needed.</summary>
    internal const int VisibleLanes = CollapsedLanes - 1;

    /// <summary>
    /// Whether there are more lanes than the collapsed banner can show.
    /// </summary>
    /// <remarks>
    /// Exactly <see cref="CollapsedLanes"/> lanes fit with no overflow row, the "+N" only appears
    /// when it would actually be hiding something, so three all-day events show as three bars, not
    /// two and a "+1". A "+1" that hides an event for no reason is worse than nothing.
    /// </remarks>
    internal static bool Overflows(int lanes) => lanes > CollapsedLanes;

    /// <summary>
    /// The lanes drawn as bars right now: all of them when expanded or when they fit, else the first
    /// <see cref="VisibleLanes"/>, with the last row reserved for the "+N" chips.
    /// </summary>
    internal static int DrawnLanes(int lanes, bool expanded) =>
        expanded || !Overflows(lanes) ? lanes : VisibleLanes;

    /// <summary>How tall the banner is, in lanes, including any "+N" row.</summary>
    internal static int BannerLanes(int lanes, bool expanded) =>
        expanded || !Overflows(lanes) ? lanes : CollapsedLanes;

    /// <summary>
    /// For each day column, how many of its all-day bars the collapsed banner is hiding.
    /// </summary>
    /// <remarks>
    /// A multi-day bar counts against <b>every</b> day it covers, it is hidden on all of them, so a
    /// three-day offsite pushed out of view adds one to three different columns. Counting it once
    /// would under-report two of them, and <b>a "+1" that should say "+2" is a lie the user cannot see
    /// through</b>: they tap, find an event nobody told them about, and stop trusting the banner.
    /// </remarks>
    internal static IReadOnlyList<int> OverflowPerDay(
        IReadOnlyList<BandSpan> bands,
        int dayCount,
        int drawnLanes)
    {
        var counts = new int[dayCount];
        foreach (var band in bands)
        {
            if (band.Lane < drawnLanes)
            {
                continue;
            }
            for (var day = band.Day; day < band.Day + band.Days && day < dayCount; day++)
            {
                if (day >= 0)
                {
                    counts[day]++;
                }
            }
        }
        return counts;
    }
}
