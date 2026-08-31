// Where the grid is, how big it is, and what a gesture does to it, one owner, one place.
//
// This is the client half of the calendar contract's first rule: the core says "day 3, minute 545,
// column 1 of 2" and carries no units at all; everything below is a multiplication. An hour has no
// height and a day no width until this file says so.
//
// **The horizontal axis is ONE continuous strip of days, not a pager over a day-scroll.** It used to
// be three coupled numbers, a week index, a page offset, and a day offset within the week, with a
// drag routed to the day strip first and the remainder pushed into the page. That decomposition was
// never visible to the user: all three moved the same days by the same pixels, and the split was
// bookkeeping. What it *did* buy was a bug. A page carried its own hour ruler, so the only coherent
// resting places were the week boundaries, so every gesture had to be resolved onto one, and a
// touchpad, which never tells an app that a gesture ended (§6), had to guess from silence and snap. A
// slow two-finger pan is mostly silence, so it snapped, over and over, and the grid rubber-banded back
// to the week under the user's fingers. Seen on a real trackpad; the video is in the PR.
//
// So the strip is now continuous and the ruler is pinned (see CalendarSurfaceDraw): the grid may come
// to rest showing Wednesday to Tuesday across a week boundary, because that frame is now a perfectly
// good frame. What it rests ON is a day, the same rule at every zoom and for every input, wheel and
// finger alike (§6). A day is the smallest unit that puts a column edge against the grid's left edge,
// so it is the least the grid can move and still look deliberate.
//
// It is a plain class, with no WinUI type anywhere in it, so the whole navigation model of the time
// grid, the scroll offsets, the zoom, the week, is unit-testable without constructing a single
// control. That is not incidental: the bugs docs/calendar.md was written from all look fine right up
// until a hand moves fast, and a rule nobody can check is a rule that comes back.
using System;

namespace Allodia.Mailcal.Calendar;

/// <summary>
/// The chrome around the grid, in pixels, everything a zoom cannot change.
/// </summary>
/// <remarks>
/// <paramref name="Lanes"/> is the <b>true</b> lane count the core stacked the all-day bands into,
/// taken across <i>every week the strip is showing</i>, not the number the banner draws: the cap is
/// applied here, against the banner's own expanded state.
/// <para>
/// Across every week, because the hour ruler is pinned and single (§6). The grid's top edge is where
/// the ruler's <c>00:00</c> begins, so it must be one height for the whole surface, two weeks either
/// side of a seam with different banner heights would leave the hour lines meeting the ruler on only
/// one of them.
/// </para>
/// </remarks>
internal readonly record struct SurfaceViewport(
    float Width,
    float Height,
    float Gutter,
    float HeaderHeight,
    float LaneHeight,
    float DividerHeight,
    int Lanes);

/// <summary>
/// The grid's pixel geometry at the current zoom.
/// </summary>
/// <remarks>
/// A week's stride is <see cref="WeekWidth"/>, seven day columns, and <b>no gutter</b>. The hour
/// ruler is not part of a week: it is drawn once, pinned to the left edge, and the days scroll past
/// it. (It used to belong to the page and slide out with it, which is why a page's stride was the
/// whole surface width, and why coming to rest mid-scroll stranded a second hour ruler in the middle
/// of the grid.)
/// </remarks>
internal readonly record struct SurfaceMetrics(
    SurfaceViewport Viewport,
    float HourHeight,
    float DayWidth,
    int BannerLanes)
{
    internal float Width => Viewport.Width;

    internal float Height => Viewport.Height;

    internal float Gutter => Viewport.Gutter;

    /// <summary>The width the day columns scroll through, the surface, less the pinned hour ruler.</summary>
    internal float DayViewport => MathF.Max(Width - Gutter, 0f);

    /// <summary>A week is seven day columns. It is the unit the strip is <i>indexed</i> in, and, for
    /// touch, the unit it lands on.</summary>
    internal float WeekWidth => DayWidth * CalendarUnits.DaysInWeek;

    internal float GridHeight => HourHeight * CalendarUnits.HoursInDay;

    internal float BannerHeight => Viewport.LaneHeight * BannerLanes;

    internal float ContentTop => Viewport.HeaderHeight + BannerHeight + Viewport.DividerHeight;

    internal float ContentHeight => MathF.Max(Height - ContentTop, 0f);

    internal float MaxScrollY => MathF.Max(GridHeight - ContentHeight, 0f);

    /// <summary>A block's rectangle, in its <b>own week's</b> coordinates, before the strip offset.</summary>
    /// <remarks>
    /// This, <see cref="BandRect"/> and <see cref="MoreRect"/> are the client's <i>whole</i>
    /// contribution to layout, and the only thing about it that can be wrong. Swap
    /// <c>Column</c> for <c>Columns</c>, or forget to offset by the day, and every block still renders
    /// perfectly plausibly, in the wrong place.
    /// <para>
    /// They are shared by the renderer <b>and</b> by the accessibility overlay, deliberately. Two
    /// copies of this arithmetic would be two chances to disagree, and a screen reader announcing an
    /// event somewhere other than where it is drawn is a bug nobody would ever see.
    /// </para>
    /// </remarks>
    internal GridRect BlockRect(BlockSpan block)
    {
        var columnWidth = DayWidth / block.Columns;
        var left = (DayWidth * block.Day) + (columnWidth * block.Column);
        var top = HourHeight * (block.StartMinutes / CalendarUnits.MinutesInHour);
        return new GridRect(
            left,
            top,
            left + columnWidth,
            top + (HourHeight * (block.Minutes / CalendarUnits.MinutesInHour)));
    }

    /// <summary>An all-day bar's rectangle, in its week's banner coordinates.</summary>
    internal GridRect BandRect(BandSpan span)
    {
        var left = DayWidth * span.Day;
        var top = Viewport.LaneHeight * span.Lane;
        return new GridRect(
            left,
            top,
            left + (DayWidth * span.Days),
            top + Viewport.LaneHeight);
    }

    /// <summary>A "+N" chip's rectangle: one column wide, in the row the last visible lane gave up.</summary>
    internal GridRect MoreRect(int day, int lane) =>
        BandRect(new BandSpan(day, 1, lane));
}

/// <summary>
/// The whole state of the time grid: where it is scrolled, how far it is zoomed, and which week.
/// </summary>
internal sealed class CalendarSurfaceState(int visibleHours, int visibleDays)
{
    /// <summary>The two zoom axes, and the rules about where they stop.</summary>
    internal CalendarZoom Zoom { get; } = new(visibleHours, visibleDays);

    /// <summary>How far down the day the grid is scrolled, in pixels.</summary>
    internal float ScrollY { get; private set; }

    /// <summary>
    /// The week the strip is <b>indexed from</b>, the one whose first day <see cref="StripX"/> is
    /// measured against, and the page whose data the grid is centred on.
    /// </summary>
    /// <remarks>
    /// Bookkeeping, not a position: the user is not "on" a week any more than a scrolled list is "on"
    /// a row. It is what names the page to pull, to cache, and to prebuild around, and it moves only
    /// when the strip's left edge crosses a week boundary (see <see cref="Rebase"/>).
    /// </remarks>
    internal int Week { get; private set; }

    /// <summary>
    /// How far into <see cref="Week"/> the strip is scrolled: the pixels between that week's first day
    /// and the left edge of the day viewport. Always in <c>[0, WeekWidth)</c>.
    /// </summary>
    /// <remarks>
    /// <b>Non-zero at rest is legal, and is the whole point of this change.</b> The days are one
    /// continuous strip and the hour ruler is pinned beside it, so a grid showing Wednesday to Tuesday
    /// across a week boundary is a coherent frame, not a half-turned page. What it must never do is
    /// leave the range: the strip is kept inside its anchor week by moving the anchor, which keeps this
    /// bounded and precise however many weeks the user scrolls (a raw pixel offset from some far-off
    /// origin would bleed float precision, and would have to be rescaled on every pinch).
    /// </remarks>
    internal float StripX { get; private set; }

    /// <summary>Whether the all-day banner is showing every lane, or capped with a per-day "+N".</summary>
    internal bool BannerExpanded { get; private set; }

    /// <summary>
    /// The column width the labels are <b>shaped</b> against, frozen for the length of a pinch, and
    /// <c>0</c> when no pinch is in flight.
    /// </summary>
    /// <remarks>
    /// A pinch moves the column width every frame, the shaper's cache is keyed on that width, and so
    /// every visible label is re-shaped from scratch sixty times a second, in the one gesture the
    /// grid is judged on. Measured on Android against a real diary: a pinch frame cost <b>3.4×</b> a
    /// swipe frame while drawing <i>half</i> as many blocks. Bucketing the width only delays the miss.
    /// <para>
    /// So the shaping width simply <b>stops moving</b> while the fingers are down. The block's
    /// rectangle still tracks the fingers every frame, as it must; it is the layout <i>inside</i> it
    /// that is held, and it is clipped to the live rectangle anyway. The cost is that a title
    /// ellipsises against the width the block had when the pinch began, invisible, because nobody
    /// reads a label while it is moving. It re-shapes when the fingers lift.
    /// </para>
    /// </remarks>
    internal float ShapedDayWidth { get; private set; }

    /// <summary>The fingers went down: pin the shaping width where it is.</summary>
    internal void BeginZoom(SurfaceMetrics m) => ShapedDayWidth = m.DayWidth;

    /// <summary>The pixel geometry the current zoom implies, inside <paramref name="viewport"/>.</summary>
    internal SurfaceMetrics Metrics(SurfaceViewport viewport) => new(
        viewport,
        // The whole surface height over the horizon, so "show me 12 hours" means the same span on a
        // laptop and on a desktop monitor, the cells just get bigger.
        Zoom.HourHeight(viewport.Height),
        Zoom.DayWidth(MathF.Max(viewport.Width - viewport.Gutter, 0f)),
        CalendarAllDay.BannerLanes(viewport.Lanes, BannerExpanded));

    // ---- The strip -------------------------------------------------------------------------------

    /// <summary>
    /// Where the strip is, in <b>weeks</b>, fractional, and independent of the zoom.
    /// </summary>
    /// <remarks>
    /// The unit an animation should think in: a pixel means something different at every zoom, and a
    /// settle that outlived a pinch would land somewhere else entirely. Whole values are week
    /// boundaries; sevenths of one are days, which is where the strip comes to rest.
    /// </remarks>
    internal float WeekPosition(SurfaceMetrics m) =>
        m.WeekWidth <= 0f ? Week : Week + (StripX / m.WeekWidth);

    /// <summary>
    /// The nearest day boundary to where the strip is now, in weeks, the one place it rests.
    /// </summary>
    /// <remarks>
    /// A day, at <b>every</b> zoom, and for every input. The strip is continuous, so the only thing a
    /// resting rule has to buy is that a column edge lines up with the grid's left edge rather than
    /// sitting a third of a column out; a day is the smallest unit that does, so it is the least the
    /// grid can move and still look deliberate.
    /// <para>
    /// It is deliberately <b>not</b> a week. Landing a week away moves the days by up to half a week
    /// the user never asked for, and inferring "the gesture ended" from a touchpad's silence in order
    /// to do it is the rubber-band this file's header describes. Half a day is small enough that the
    /// correction reads as settling rather than as being overruled.
    /// </para>
    /// </remarks>
    internal float NearestDay(SurfaceMetrics m) =>
        m.WeekWidth <= 0f
            ? Week
            : MathF.Round(WeekPosition(m) * CalendarUnits.DaysInWeek) / CalendarUnits.DaysInWeek;

    /// <summary>Puts the strip at an absolute (fractional) week position.</summary>
    internal void ScrollToWeeks(float weeks, SurfaceMetrics m)
    {
        if (m.WeekWidth <= 0f)
        {
            return;
        }
        var anchor = (int)MathF.Floor(weeks);
        Week = anchor;
        StripX = (weeks - anchor) * m.WeekWidth;
    }

    /// <summary>
    /// Scrolls the days. <paramref name="dx"/> is the finger's movement: left brings on later days.
    /// </summary>
    /// <remarks>
    /// One axis, one line. There is no day-strip-first-then-the-week hand-off any more, because there
    /// is nothing to hand off <i>to</i>: the days are one strip and a week is just a distance along it.
    /// Reversing a drag mid-scroll needs no special case either, the strip simply goes back the way it
    /// came.
    /// </remarks>
    internal void PanX(float dx, SurfaceMetrics m)
    {
        StripX -= dx;
        Rebase(m);
    }

    /// <summary>Scrolls the hours. <paramref name="dy"/> is the finger's movement: down reveals
    /// earlier hours.</summary>
    internal void PanY(float dy, SurfaceMetrics m) =>
        ScrollY = Math.Clamp(ScrollY - dy, 0f, m.MaxScrollY);

    /// <summary>Puts the strip at <paramref name="x"/> pixels into the anchor week, the fling's step.</summary>
    internal void StripTo(float x, SurfaceMetrics m)
    {
        StripX = x;
        Rebase(m);
    }

    /// <summary>Frames the strip on a <paramref name="column"/> of the anchor week, how it opens.</summary>
    internal void FrameColumn(int column, SurfaceMetrics m)
    {
        StripX = m.DayWidth * column;
        Rebase(m);
    }

    /// <summary>Drops the strip back to the origin week's first day, a view switch, or "back to today".</summary>
    internal void ResetWeek()
    {
        Week = 0;
        StripX = 0f;
    }

    /// <summary>
    /// Whether the strip is showing any of the week <i>after</i> its anchor, i.e. straddling a seam.
    /// </summary>
    /// <remarks>
    /// Which is what decides the banner's height, and which pages have to be drawn and <b>spoken</b>.
    /// At rest on a boundary this is false and the grid is one week, exactly as it always was.
    /// </remarks>
    internal bool SecondWeekVisible(SurfaceMetrics m) =>
        m.WeekWidth > 0f && StripX + m.DayViewport > m.WeekWidth + 0.5f;

    /// <summary>
    /// Keeps <see cref="StripX"/> inside its anchor week by moving the anchor, the strip's only
    /// discontinuity, and one nothing can see.
    /// </summary>
    /// <remarks>
    /// A week is added to <see cref="Week"/> and its width taken off <see cref="StripX"/>, which leaves
    /// every day of the strip at exactly the same pixel: the two terms cancel in
    /// <c>(k - Week) * WeekWidth - StripX</c>. All that changes is which week the grid calls "the one
    /// it is on", and so which pages it holds, prebuilds and speaks.
    /// <para>
    /// A loop rather than an <c>if</c>: a hard fling can cross more than one week between two frames.
    /// </para>
    /// </remarks>
    private void Rebase(SurfaceMetrics m)
    {
        var week = m.WeekWidth;
        if (week <= 0f)
        {
            return;
        }
        while (StripX >= week)
        {
            StripX -= week;
            Week++;
        }
        while (StripX < 0f)
        {
            StripX += week;
            Week--;
        }
    }

    // ---- Zooming ---------------------------------------------------------------------------------

    /// <summary>
    /// One frame of a pinch: both axes, each by its own component of the spread.
    /// </summary>
    /// <remarks>
    /// <paramref name="focusX"/> and <paramref name="focusY"/> are the fingers' midpoint <b>relative
    /// to the content viewport</b>, past the hour ruler, below the banner. Anchoring on them is what
    /// keeps the content under the fingers still; without it the offset stays fixed in <i>pixels</i>
    /// while the scale changes, so the same offset maps to a different time and the grid slides out
    /// from under the user's hand.
    /// <para>
    /// Each axis is corrected by <b>the factor its zoom actually applied</b>, not the one it was asked
    /// for. At a clamp that is <c>1</c>, and correcting by the requested factor there would drag the
    /// grid on every further frame of a pinch that has nowhere left to go.
    /// </para>
    /// <para>
    /// The day axis is corrected with <b>no bound of its own</b>: the strip is infinite, so a pinch
    /// that pulls the focus day back past the anchor week's first day simply rebases onto the week
    /// before it. (It used to clamp at the week's edge, which is what made a pinch at the end of a week
    /// creep the days sideways.)
    /// </para>
    /// <para>
    /// Note what this does <b>not</b> do: pan. The fingers' midpoint travelling across the glass moves
    /// nothing.
    /// </para>
    /// </remarks>
    internal void Pinch(float xScale, float yScale, float focusX, float focusY, SurfaceViewport viewport)
    {
        var hours = Zoom.PinchVertical(yScale);
        if (hours != 1f)
        {
            ScrollY = FocalPreserving(ScrollY, focusY, hours);
        }
        var days = Zoom.PinchHorizontal(xScale);
        if (days != 1f)
        {
            StripX = FocalPreserving(StripX, focusX, days);
        }
        // The zoom just moved the week's width and the day's height; the strip re-anchors and the
        // hours are put back inside the day.
        ClampScroll(Metrics(viewport));
    }

    /// <summary>
    /// Snaps the day axis to the zoom level the pinch settled on, and returns that level.
    /// </summary>
    /// <remarks>
    /// The snap itself, and the reason it is to the settled <i>level's</i> columns rather than to the
    /// rounded count, lives in <see cref="CalendarZoom.SettleDays"/>. All this adds is the re-anchor:
    /// the columns just changed width, so the strip's offset means a different day than it did.
    /// </remarks>
    internal CalendarMode SettleZoom(SurfaceViewport viewport)
    {
        Zoom.SettleDays();
        ClampScroll(Metrics(viewport));
        // The fingers are up: the labels may re-shape against the width they actually have.
        ShapedDayWidth = 0f;
        return CalendarModes.ForColumns(Zoom.SettledDays());
    }

    /// <summary>The whole-hour horizon to persist once the fingers lift.</summary>
    internal int SettledHours() => Zoom.SettledHours();

    // ---- Everything else -------------------------------------------------------------------------

    internal void ToggleBanner() => BannerExpanded = !BannerExpanded;

    /// <summary>Re-seeds the horizon (on load, or when the settings screen changes it).</summary>
    internal void ResetHours(int hours) => Zoom.ResetHours(hours);

    /// <summary>Re-seeds the day axis (on load, or when a shape is picked from the menu).</summary>
    internal void ResetDays(int days) => Zoom.ResetDays(days);

    internal void ScrollTo(float y, SurfaceMetrics m) => ScrollY = Math.Clamp(y, 0f, m.MaxScrollY);

    /// <summary>
    /// Puts the grid back inside its bounds, after a zoom, or a change of banner height.
    /// </summary>
    /// <remarks>
    /// Must be called after every layout, not only from a pinch: the vertical bound moves with no
    /// finger on the glass. Swiping to a week with fewer all-day lanes grows the grid taller and
    /// shrinks <see cref="SurfaceMetrics.MaxScrollY"/>, leaving the day scrolled past midnight,
    /// showing a strip of nothing. That was a real regression.
    /// <para>
    /// The <b>horizontal</b> axis has no bound to clamp to any more, the strip runs forever, so all
    /// it needs is its anchor kept honest. (The old day-scroll <i>did</i> have one, and picking a wider
    /// shape from the menu used to collapse it to zero and draw the week a thousand pixels off to the
    /// left: the grid came up blank, which looked like a rendering crash and was really a stale
    /// offset. A strip cannot have that bug, but it can be left mid-week by a zoom, so it re-anchors.)
    /// </para>
    /// </remarks>
    internal void ClampScroll(SurfaceMetrics m)
    {
        ScrollY = Math.Clamp(ScrollY, 0f, m.MaxScrollY);
        Rebase(m);
    }

    /// <summary>
    /// The offset that keeps whatever was under <paramref name="focus"/> exactly under
    /// <paramref name="focus"/>, after the content has been scaled by <paramref name="factor"/>.
    /// </summary>
    /// <remarks>
    /// The content point under the fingers sits at <c>offset + focus</c> pixels along the content.
    /// Scaling moves it to <c>(offset + focus) * factor</c>; putting it back under the same finger
    /// means scrolling to that, less the finger's own offset in the viewport. Works for either axis,
    /// which is what lets a diagonal pinch anchor on a single point rather than fighting itself. The
    /// caller bounds it: the hours clamp to the day, the strip re-anchors.
    /// </remarks>
    internal static float FocalPreserving(float offset, float focus, float factor) =>
        ((offset + focus) * factor) - focus;
}
