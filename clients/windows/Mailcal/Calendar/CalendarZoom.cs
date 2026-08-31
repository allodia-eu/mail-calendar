// Pinch-to-zoom: how much of the day, and how many of the week's days, the grid shows at once.
//
// The model is Samsung's, and it is the one that never jumps: **a page is a week**. Zooming
// horizontally does not switch to a different *view*; it just narrows the columns, and the days stay
// exactly where they are.
//
// One half of Samsung's model this client has since dropped: the week was also a *wall*, you could
// not scroll out of it, you paged to the next one. The days are now one continuous strip and a scroll
// runs straight through the boundary (docs/calendar.md §6). The week remains the unit the core is
// queried in, the unit a page is cached as, and the unit a touch swipe lands on; it is no longer the
// unit you are trapped in.
//
// That is why the earlier Android design felt broken, and it is written down here so Windows does
// not rediscover it: snapping a pinch to a Monday-aligned "week view" CANNOT keep an arbitrary
// three-day window on screen, a user reading Sunday, Monday and Tuesday who pinched outwards was
// shown the previous Monday-to-Sunday, and two of the three days they were reading vanished. Here
// the days never move; only their width does.
//
// The horizon (hours) and the column count are both persisted as core settings, so all clients open
// the same way. But neither an hour nor a day has a *size* until this client multiplies, the core's
// geometry is unit-free, so the zoom itself lives here, and only the settled values go back.
//
// See docs/calendar.md §2 and §8.
using System;

namespace Allodia.Mailcal.Calendar;

/// <summary>
/// How much of the day, and how many of the week's days, are on screen, and what a pinch does to
/// each.
/// </summary>
/// <remarks>
/// Both are <b>fractional</b>. A pinch is continuous, and rounding mid-gesture would make the grid
/// jump between whole hours (or whole columns) instead of tracking the fingers, which is the
/// difference between "buttery" and "broken". Only the settled values are whole, and only those
/// escape.
/// </remarks>
internal sealed class CalendarZoom
{
    // The same clamps the core enforces (mailcal-account: MIN/MAX_VISIBLE_HOURS). Held here too so
    // the live gesture is bounded as it happens, rather than snapping back when the core rejects it.
    // A pinch runs off the end of its own gesture constantly.
    internal const float MinVisibleHours = 4f;
    internal const float MaxVisibleHours = 24f;

    /// <summary>You can zoom to a single day, and out to the whole week. Never further: the week is
    /// the page.</summary>
    internal const float MinVisibleDays = 1f;

    /// <summary>The whole week, the page itself.</summary>
    internal const float MaxVisibleDays = CalendarUnits.DaysInWeek;

    internal CalendarZoom(int visibleHours, int visibleDays)
    {
        VisibleHours = Math.Clamp(visibleHours, MinVisibleHours, MaxVisibleHours);
        VisibleDays = Math.Clamp(visibleDays, MinVisibleDays, MaxVisibleDays);
    }

    /// <summary>Hours of the day currently on screen. Fractional while the fingers are down.</summary>
    internal float VisibleHours { get; private set; }

    /// <summary>Day columns of the week currently on screen. Fractional: half a column may show.</summary>
    internal float VisibleDays { get; private set; }

    /// <summary>
    /// Applies one frame of a vertical pinch, and returns <b>the factor the hour height actually grew
    /// by</b>, which is not the factor asked for once the zoom hits its clamp.
    /// </summary>
    /// <remarks>
    /// The caller needs the real one: it corrects the scroll offset by exactly this, to keep the
    /// content under the fingers still. Correcting by the <i>requested</i> factor at the end of the
    /// range would drag the grid on every further frame of a pinch that has nowhere left to go, and
    /// let an exhausted hour axis drag the day axis to a halt mid-diagonal.
    /// <para>
    /// <paramref name="zoom"/> &gt; 1 is fingers spreading apart, which must show <i>fewer</i> hours
    /// (zoom in), so it divides. Get that backwards and the grid zooms out when the user pinches in,
    /// which feels broken instantly.
    /// </para>
    /// </remarks>
    internal float PinchVertical(float zoom)
    {
        if (zoom <= 0f)
        {
            return 1f;
        }
        var before = VisibleHours;
        VisibleHours = Math.Clamp(VisibleHours / zoom, MinVisibleHours, MaxVisibleHours);
        // An hour got taller by exactly as much as the horizon got shorter.
        return before / VisibleHours;
    }

    /// <summary>The same, for the day axis: spreading sideways shows fewer, wider days.</summary>
    internal float PinchHorizontal(float zoom)
    {
        if (zoom <= 0f)
        {
            return 1f;
        }
        var before = VisibleDays;
        VisibleDays = Math.Clamp(VisibleDays / zoom, MinVisibleDays, MaxVisibleDays);
        return before / VisibleDays;
    }

    /// <summary>The whole-hour horizon to persist once the fingers lift.</summary>
    internal int SettledHours() =>
        Math.Clamp((int)MathF.Round(VisibleHours), (int)MinVisibleHours, (int)MaxVisibleHours);

    /// <summary>The whole-column count to persist once the fingers lift.</summary>
    internal int SettledDays() =>
        Math.Clamp((int)MathF.Round(VisibleDays), (int)MinVisibleDays, (int)MaxVisibleDays);

    /// <summary>Re-seeds the horizon (on load, or when the settings screen changes it).</summary>
    internal void ResetHours(int hours) =>
        VisibleHours = Math.Clamp(hours, MinVisibleHours, MaxVisibleHours);

    /// <summary>Re-seeds the day axis (on load, or when a shape is picked from the menu).</summary>
    internal void ResetDays(int days) =>
        VisibleDays = Math.Clamp(days, MinVisibleDays, MaxVisibleDays);

    /// <summary>
    /// Snaps the day axis to the <b>zoom level</b> the pinch settled on, once the fingers lift.
    /// </summary>
    /// <remarks>
    /// The shape is a persisted core setting with exactly four values (<see cref="CalendarLayout"/>),
    /// so a zoom that comes to rest between two rungs has nothing to save, and the columns would not
    /// divide the week, leaving a day hanging off the side of the screen.
    /// <para>
    /// It snaps to the settled <b>level's</b> columns, not to <see cref="SettledDays"/>, and the
    /// difference is a real bug: a pinch outwards from the week lands on ~6.4 columns, which rounds to
    /// <i>6</i> while the level it maps to is the whole WEEK, of <i>7</i>. The grid would then draw a
    /// seven-day week at one-sixth of the viewport per column, in the one view that is supposed to fit
    /// exactly.
    /// </para>
    /// <para>
    /// On Android that overhang is worse than untidy, it is a horizontal scroll nested inside the
    /// pager, and a nested scroll takes the drag <i>first</i>, so the swipe that should turn the week is
    /// spent sliding along the current one. Here there is no pager and nothing nested: the days are one
    /// strip (docs/calendar.md §6), so the rung is kept for the persisted shape and the tidy columns,
    /// not to protect the swipe.
    /// </para>
    /// </remarks>
    internal void SettleDays() => ResetDays(CalendarModes.ForColumns(SettledDays()).Columns());

    /// <summary>
    /// How tall one hour is, in pixels, given the height of the grid's viewport.
    /// </summary>
    /// <remarks>
    /// The bridge from the core's unit-free geometry to pixels: every block's vertical offset and
    /// height is a multiple of it.
    /// </remarks>
    internal float HourHeight(float viewport) => viewport / VisibleHours;

    /// <summary>How wide one day column is, given the width of the grid's viewport.</summary>
    internal float DayWidth(float viewport) => viewport / VisibleDays;
}
