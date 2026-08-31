// What the calendar is showing and where, the state a swipe, a view switch, and "back to today"
// all move.
//
// The Rust core never tracks where the user is: `calendar_range(from, columns)` is a pull with an
// argument, so the *client* owns the anchor. That makes this the whole of the navigation model, and
// it is a plain class rather than a knot of view state so the page<->date mapping is unit-testable
// without constructing a single WinUI element.
using System;
using System.Collections.Generic;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>
/// The shape the calendar is drawn in.
/// </summary>
/// <remarks>
/// The four grid shapes are <b>not four views</b>. They are four zoom levels of one grid: a page is
/// always a whole week, and the "view" is just how many of its seven columns are on screen. That is
/// what makes zooming smooth, the days never move, only their width changes.
/// <para>
/// Snapping a zoom to a differently-anchored view is what made the first Android design jump: a
/// Monday-aligned week cannot contain an arbitrary three-day window, so a user reading Sunday to
/// Tuesday who pinched outwards was shown the <i>previous</i> Monday-to-Sunday, and two of their
/// three days vanished. See docs/calendar.md §2.
/// </para>
/// </remarks>
internal enum CalendarMode
{
    /// <summary>One column.</summary>
    Day,

    /// <summary>Three columns.</summary>
    ThreeDay,

    /// <summary>Five columns.</summary>
    WorkWeek,

    /// <summary>All seven, the page, exactly filled.</summary>
    Week,

    /// <summary>The month grid: a different layout, with no hour axis and no zoom.</summary>
    Month,

    /// <summary>The agenda list.</summary>
    Agenda,
}

/// <summary>The zoom levels of the time grid, and how they map to columns and to the persisted setting.</summary>
internal static class CalendarModes
{
    /// <summary>The four zoom levels of the one time grid. The month and the agenda are not among them.</summary>
    internal static readonly IReadOnlyList<CalendarMode> Grid =
    [
        CalendarMode.Day,
        CalendarMode.ThreeDay,
        CalendarMode.WorkWeek,
        CalendarMode.Week,
    ];

    /// <summary>
    /// The shape the grid opens in.
    /// </summary>
    /// <remarks>
    /// The week, and not a subset of it: it is the one shape that answers "what am I doing?" without
    /// being scrolled, and it is what every other calendar opens on. (This is only the <i>default</i>,
    /// the shape is a persisted core setting, so the grid reopens however the user last left it.)
    /// <para>
    /// It used to be load-bearing for a different reason, and that reason is gone: a sub-week zoom left
    /// the rest of the page hanging off the side of the screen, which was a horizontal scroll nested
    /// inside the pager, and a nested scroll takes the drag <i>first</i>, so a swipe meant to turn the
    /// week was spent sliding along the week you were already on. There is no pager and no nesting now;
    /// the days are one strip and a week is a distance along it (docs/calendar.md §6).
    /// </para>
    /// </remarks>
    internal const CalendarMode Default = CalendarMode.Week;

    /// <summary>Whether this is the time grid, at any zoom.</summary>
    internal static bool IsGrid(this CalendarMode mode) => mode switch
    {
        CalendarMode.Day or CalendarMode.ThreeDay or CalendarMode.WorkWeek or CalendarMode.Week => true,
        _ => false,
    };

    /// <summary>Whether this is the month grid, a different layout, with no hour axis and no zoom.</summary>
    internal static bool IsMonth(this CalendarMode mode) => mode == CalendarMode.Month;

    /// <summary>How many of the week's seven columns this zoom level shows.</summary>
    internal static int Columns(this CalendarMode mode) => mode switch
    {
        CalendarMode.Day => 1,
        CalendarMode.ThreeDay => 3,
        CalendarMode.WorkWeek => 5,
        CalendarMode.Week => CalendarUnits.DaysInWeek,
        _ => 0,
    };

    /// <summary>
    /// The columns to seed the day-axis zoom with.
    /// </summary>
    /// <remarks>
    /// The month and the agenda have no columns of their own (<see cref="Columns"/> is 0, and dividing
    /// the viewport by it would be an infinity). But the grid is still <i>there</i>, one menu click
    /// away, so it is seeded with the whole week, which is what it should show when the user returns.
    /// </remarks>
    internal static int GridColumns(this CalendarMode mode) =>
        mode.IsGrid() ? mode.Columns() : CalendarUnits.DaysInWeek;

    /// <summary>
    /// The zoom level showing <paramref name="columns"/> of the week's days, the inverse of
    /// <see cref="Columns"/>.
    /// </summary>
    /// <remarks>
    /// A settled pinch often lands exactly between two rungs (two columns is as close to one as to
    /// three). The tie is broken <b>towards more days</b>, deliberately, rather than by whatever
    /// order a collection happens to iterate in: showing a day the user didn't ask for is a smaller
    /// sin than hiding one they did.
    /// </remarks>
    internal static CalendarMode ForColumns(int columns)
    {
        var best = CalendarMode.ThreeDay;
        var bestDistance = int.MaxValue;
        foreach (var mode in Grid)
        {
            var distance = Math.Abs(mode.Columns() - columns);
            // Strictly closer wins; an equal distance wins only with MORE columns.
            if (distance < bestDistance ||
                (distance == bestDistance && mode.Columns() > best.Columns()))
            {
                best = mode;
                bestDistance = distance;
            }
        }
        return best;
    }

    /// <summary>The persisted core setting this mode is stored as, so the calendar reopens as it was left.</summary>
    internal static CalendarLayout ToLayout(this CalendarMode mode) => mode switch
    {
        CalendarMode.Day => CalendarLayout.Day,
        CalendarMode.ThreeDay => CalendarLayout.ThreeDay,
        CalendarMode.WorkWeek => CalendarLayout.WorkWeek,
        CalendarMode.Week => CalendarLayout.Week,
        CalendarMode.Month => CalendarLayout.Month,
        _ => CalendarLayout.Agenda,
    };

    /// <summary>The mode a persisted <see cref="CalendarLayout"/> restores to.</summary>
    internal static CalendarMode ToMode(this CalendarLayout layout) => layout switch
    {
        CalendarLayout.Day => CalendarMode.Day,
        CalendarLayout.ThreeDay => CalendarMode.ThreeDay,
        CalendarLayout.WorkWeek => CalendarMode.WorkWeek,
        CalendarLayout.Week => CalendarMode.Week,
        CalendarLayout.Month => CalendarMode.Month,
        _ => CalendarMode.Agenda,
    };
}

/// <summary>
/// Maps pages to anchor dates, and moves the origin when the user switches view or jumps home.
/// </summary>
/// <remarks>
/// <paramref name="today"/> seeds the origin. The origin only moves on a deliberate jump, a view
/// switch or "back to today", never on a swipe, because a swipe is just a different page over the
/// same origin.
/// <para>
/// <paramref name="alignToWeek"/> is the core's <c>week_start_date</c>. A grid page is a whole week,
/// and the week is <b>aligned</b>, so a seat on today opens on the week's first day (Monday, by
/// default), not on today itself. Get this wrong and a 7-column week beginning on a Tuesday shows
/// Tuesday under the first heading; the whole scroll-to-today model (<see cref="TodayColumn"/>)
/// already assumes an aligned origin. Alignment is applied only when the origin is <i>seated</i>, a
/// seat on today, a jump home, a view switch, <b>never on a zoom</b>, which is exactly the jump §3
/// forbids. Left unset it is the identity, so a test that does not care about alignment gets the raw
/// dates it passes in; the real client always injects the core's, because deriving the week start
/// client-side is how the columns drift (docs/calendar.md §3).
/// </para>
/// </remarks>
internal sealed class CalendarPager(
    DateOnly today,
    CalendarMode mode = CalendarModes.Default,
    Func<DateOnly, DateOnly>? alignToWeek = null)
{
    private static readonly Func<DateOnly, DateOnly> Identity = date => date;

    // The core's week-start aligner, or the identity. Applied only to grid modes: the month anchors
    // on the 1st of a month (aligning its seed to a Monday could cross a month boundary), and the
    // agenda has no columns to align.
    private readonly Func<DateOnly, DateOnly> _align = alignToWeek ?? Identity;

    /// <summary>The shape being drawn.</summary>
    internal CalendarMode Mode { get; private set; } = mode;

    /// <summary>The date page 0 shows, week-aligned for a grid mode (§3).</summary>
    internal DateOnly Origin { get; private set; } =
        mode.IsGrid() ? (alignToWeek ?? Identity)(today) : today;

    /// <summary>Bumped whenever <see cref="Origin"/> moves, so the surface re-centres on it.</summary>
    internal int ResetToken { get; private set; }

    /// <summary>
    /// The anchor date <paramref name="page"/> shows, the first day the core's query is asked for.
    /// </summary>
    /// <remarks>
    /// A grid page is a <b>whole week</b>, whatever the zoom, that is what the core is asked for, what
    /// the cache is keyed on, and the unit the strip counts in. It is no longer a wall: the days are one
    /// continuous strip and a scroll runs straight through a week boundary into the next (§6). What the
    /// week still is, is the <i>page</i>: the thing pulled, painted and cached as a unit, and the thing
    /// a touch swipe lands on. The zoom never changes what this returns, only how many of that week's
    /// columns fit on screen.
    /// <para>
    /// The month is the one shape a day-stride cannot express: months are 28–31 days long, so striding
    /// by a constant would drift off the month within a year. It pages by calendar month instead, and
    /// anchors on the 1st so adding months from (say) the 31st cannot silently clamp to the 28th and
    /// lose a day each time.
    /// </para>
    /// </remarks>
    internal DateOnly AnchorFor(int page) => Mode.IsMonth()
        ? new DateOnly(Origin.Year, Origin.Month, 1).AddMonths(page)
        : Origin.AddDays(page * CalendarUnits.DaysInWeek);

    /// <summary>Switches shape, keeping the period the user is looking at.</summary>
    /// <remarks>
    /// The new origin is re-aligned when the target is a grid, switching from the month (which
    /// anchors on the 1st) to a grid must open on that date's <i>week</i>, not on the 1st mid-week.
    /// A grid-to-grid switch is a no-op for alignment: the anchor is already a week start, and
    /// <see cref="_align"/> is idempotent.
    /// </remarks>
    internal void SetMode(CalendarMode next, int currentPage)
    {
        if (next == Mode)
        {
            return;
        }
        var anchor = AnchorFor(currentPage);
        Mode = next;
        Origin = next.IsGrid() ? _align(anchor) : anchor;
        ResetToken++;
    }

    /// <summary>
    /// Changes the <b>zoom level</b> without touching the origin.
    /// </summary>
    /// <remarks>
    /// The difference from <see cref="SetMode"/> matters: that one re-origins on the page you are on,
    /// which is right for a menu choice and wrong for a pinch. A zoom must leave the week exactly
    /// where it is, the columns only get wider. Week alignment is a deliberate act, never a
    /// side-effect of a zoom (docs/calendar.md §3).
    /// </remarks>
    internal void SetZoom(CalendarMode next)
    {
        if (next == Mode || !next.IsGrid())
        {
            return;
        }
        Mode = next;
    }

    /// <summary>Re-centres on <paramref name="date"/>, the "back to today" affordance.</summary>
    /// <remarks>
    /// A deliberate seat, so the week is re-aligned (§3): jumping home in week view opens on the
    /// week's first day with today in its own column, not on a week that begins on today.
    /// </remarks>
    internal void JumpTo(DateOnly date)
    {
        Origin = Mode.IsGrid() ? _align(date) : date;
        ResetToken++;
    }

    /// <summary>
    /// Which column of its own week <paramref name="date"/> sits in, <c>0</c> for the first.
    /// </summary>
    /// <remarks>
    /// The grid scrolls here when it opens, and when the user asks to come home. Both used to scroll
    /// to column 0, the first day of the week, which is not where today is on any day but the
    /// first: on a Sunday, with a Monday-start week, today is the <i>last</i> column and was six of
    /// them off the edge of the screen. The app opened on a week that did not visibly contain today.
    /// <para>
    /// <paramref name="weekStart"/> comes from the <b>core</b> (<c>week_start_date</c>), so this
    /// cannot disagree with the columns the core laid out. Deriving it from the device locale here is
    /// how the two drift apart, and every column shifting means the user reads Tuesday's meetings
    /// under Monday's heading (docs/calendar.md §3).
    /// </para>
    /// </remarks>
    internal static int TodayColumn(DateOnly date, DateOnly weekStart) =>
        Math.Clamp(date.DayNumber - weekStart.DayNumber, 0, CalendarUnits.DaysInWeek - 1);

    /// <summary>
    /// Which column the grid's left edge frames on when it opens or jumps home.
    /// </summary>
    /// <remarks>
    /// A deliberate, cross-platform product decision, kept identical on Android:
    /// <list type="bullet">
    /// <item>
    /// <b>Work week</b> is framed from the week's <i>first day</i>, not from today: "work week" means
    /// Monday–Friday, so it always opens on the aligned week start and shows five days, whatever day
    /// today is. (Reach the weekend by scrolling on.)
    /// </item>
    /// <item>
    /// <b>Week</b> is framed from the week's first day too, and this is <b>load-bearing</b>: the seven
    /// columns fill the screen, so framing on today would open the grid on a week that <i>begins</i> on
    /// today, a Tuesday under the first heading, which is the very thing §3's alignment exists to
    /// prevent. It used to fall out of a clamp (the day axis could not scroll within a whole-week zoom,
    /// so a non-zero framing column was quietly clipped to zero). The strip has no such bound any more,
    /// it runs through the weeks, so what was an accident of the geometry is now said out loud.
    /// </item>
    /// <item>
    /// <b>Day</b> shows today. <b>3-day</b> shows today plus the next two, today at the left edge, and
    /// on a Sunday that now genuinely means Sunday, Monday, Tuesday, running across the week boundary.
    /// (It used to clamp back to Friday–Sunday, because the days could not leave their week.)
    /// </item>
    /// </list>
    /// </remarks>
    internal static int FramingColumn(CalendarMode mode, DateOnly today, DateOnly weekStart) =>
        mode is CalendarMode.WorkWeek or CalendarMode.Week ? 0 : TodayColumn(today, weekStart);
}
