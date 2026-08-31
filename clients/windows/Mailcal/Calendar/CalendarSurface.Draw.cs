// The weeks the strip is showing, and the frame that draws them.
//
// Split from CalendarSurface.cs to stay under the repo's 500-line file cap (AGENTS.md), and split
// along the seam that matters: that file is the *pointer* and the *clock*; this one is the *page*.
// (What a screen reader hears is CalendarSurface.Spoken.cs, the same grid, said out loud, §4.)
//
// **Two weeks, not five.** The grid used to hold five live pages and draw all of them, because a page
// turn was committed the instant it was decided and the pixels were allowed to lag up to two whole
// pages behind the week they had already landed on, so the grid was sliding *through* weeks it had to
// be able to draw. The strip has no such thing as a lag: the anchor week is re-based the moment the
// left edge crosses a boundary, so the strip offset is always inside its own week, and the viewport is
// never wider than a week. Exactly one seam can be on screen, so exactly two weeks can be.
//
// (The pages held *ready* are still a wider halo, see PrebuildHalo. That is about not building a
// dense week's paint inside a fling, and it has nothing to do with how many are drawn.)
using System;
using System.Collections.Generic;
using System.Globalization;
using Allodia.Mailcal.Services;
using Microsoft.Graphics.Canvas;
using Microsoft.UI.Xaml.Automation.Peers;

namespace Allodia.Mailcal.Calendar;

internal sealed partial class CalendarSurface
{
    /// <summary>The anchor week, painted. Never null once the control has drawn once.</summary>
    private PagePaint Current => Page(_state.Week);

    /// <summary>
    /// The chrome around the grid, at this instant.
    /// </summary>
    /// <remarks>
    /// Read <b>fresh</b> on every event and every frame rather than captured: a pinch changes the zoom,
    /// which moves every bound the pan clamps against, mid-gesture.
    /// <para>
    /// The lane count is the <b>largest of the weeks on screen</b>, not the anchor week's own, and that
    /// is what the pinned hour ruler costs. The grid's <c>00:00</c> is where the ruler's <c>00:00</c> is,
    /// so the banner above it must be one height across the whole surface; give each week its own and
    /// the hour lines would meet the ruler on one side of a seam and miss it on the other. Resting on a
    /// week boundary there is no seam, and this is that week's own lane count, exactly as before.
    /// </para>
    /// <para>
    /// The probe is lanes-free on purpose: the horizontal geometry (the day's width, the week's width,
    /// the viewport the strip scrolls through) does not depend on the banner's height, so it can be
    /// measured before the banner's height is known, which is what breaks what would otherwise be a
    /// circular definition.
    /// </para>
    /// </remarks>
    private SurfaceViewport Viewport()
    {
        var m = _state.Metrics(Chrome(0));
        var lanes = Page(_state.Week).Lanes;
        if (_state.SecondWeekVisible(m))
        {
            lanes = Math.Max(lanes, Page(_state.Week + 1).Lanes);
        }
        return Chrome(lanes);
    }

    private SurfaceViewport Chrome(int lanes) => new(
        Width: (float)ActualWidth,
        Height: (float)ActualHeight,
        Gutter: SurfaceTheme.Gutter,
        HeaderHeight: SurfaceTheme.HeaderHeight,
        LaneHeight: SurfaceTheme.LaneHeight,
        DividerHeight: SurfaceTheme.DividerHeight,
        Lanes: lanes);

    /// <summary>Where week <paramref name="week"/>'s first day sits, in day-viewport coordinates.</summary>
    /// <remarks>
    /// The one line of arithmetic the whole strip is made of. Negative for the week that has partly
    /// scrolled off to the left, which is an ordinary resting state now, not a gesture in flight.
    /// </remarks>
    private float LeftOf(int week, SurfaceMetrics m) =>
        ((week - _state.Week) * m.WeekWidth) - _state.StripX;

    /// <summary>
    /// One week, painted, from the cache, or pulled from the core and painted now.
    /// </summary>
    /// <remarks>
    /// The cache is keyed on the week and thrown away wholesale when the data, the theme or the clock
    /// format changes. A <b>zoom is deliberately not in that list</b>, which is the whole point:
    /// pinching cannot invalidate this, so a pinch never rebuilds a page.
    /// </remarks>
    private PagePaint Page(int week)
    {
        if (_pages.TryGetValue(week, out var hit))
        {
            return hit;
        }
        if (PageFor is null || AnchorFor is null)
        {
            return PagePaint.Empty;
        }

        _trace.Painted();
        var anchor = AnchorFor(week);
        var page = PageFor(anchor, CalendarUnits.DaysInWeek)
            .ToPaint(_theme.Dark, Use24Hour, CultureInfo.CurrentCulture);
        _pages[week] = page;

        // A diagnostic line for the page the user is actually looking at, the one that answers an
        // "empty calendar" report from the field: was the week materialized, and how much was on it?
        // Counts and a date only, never content (docs/logging.md). Only the anchor week, so a scroll's
        // neighbour pulls do not flood the log; a fresh pull of a new anchor week is a cache miss, so
        // this is roughly one line per week the user visits, not one per frame.
        if (week == _state.Week)
        {
            Log.Info($"cal: page {anchor:yyyy-MM-dd} blocks={page.Blocks.Count} " +
                $"bands={page.Bands.Count} materialized={page.IsMaterialized}");
        }
        return page;
    }

    /// <summary>Drops the pages the grid can no longer reach, so the cache cannot grow without bound.</summary>
    private void TrimPages()
    {
        if (_pages.Count <= (PrebuildHalo * 2) + 1)
        {
            return;
        }
        var stale = new List<int>();
        foreach (var week in _pages.Keys)
        {
            if (Math.Abs(week - _state.Week) > PrebuildHalo)
            {
                stale.Add(week);
            }
        }
        foreach (var week in stale)
        {
            _pages.Remove(week);
        }
    }

    /// <summary>
    /// Paints one not-yet-built page within the prebuild halo, nearest first; returns whether it built
    /// one.
    /// </summary>
    /// <remarks>
    /// Called only on idle frames (§7). One page per call, ahead-biased, so the cost of building a
    /// busy week's paint is spread across a few invisible at-rest frames instead of landing in a fling.
    /// It presents nothing, <see cref="Page"/> only fills the cache.
    /// </remarks>
    private bool PrebuildAhead()
    {
        if (PageFor is null || AnchorFor is null)
        {
            return false;
        }
        for (var d = 0; d <= PrebuildHalo; d++)
        {
            if (!_pages.ContainsKey(_state.Week + d))
            {
                Page(_state.Week + d);
                return true;
            }
            if (d != 0 && !_pages.ContainsKey(_state.Week - d))
            {
                Page(_state.Week - d);
                return true;
            }
        }
        return false;
    }

    /// <summary>
    /// The weeks on screen, and where each one's first day sits.
    /// </summary>
    /// <remarks>
    /// At most two, and only ever the anchor week and the one after it: the strip offset is kept inside
    /// its own week (<see cref="CalendarSurfaceState.StripX"/>), and the viewport is never wider than a
    /// week, a zoom shows <i>fewer</i> of the seven columns, never more. Reused rather than allocated,
    /// because this runs on every frame of a fling.
    /// </remarks>
    private void BuildStrip(SurfaceMetrics m, DateOnly today)
    {
        _strip.Clear();
        if (m.WeekWidth <= 0f)
        {
            return;
        }
        var last = _state.SecondWeekVisible(m) ? 1 : 0;
        for (var relative = 0; relative <= last; relative++)
        {
            var week = _state.Week + relative;
            var page = Page(week);
            if (page.Headings.Count == 0)
            {
                continue;
            }
            _strip.Add(new StripPage(page, LeftOf(week, m), IndexOfToday(page, today)));
        }
    }

    /// <summary>
    /// One frame, drawn into our own swapchain, and presented.
    /// </summary>
    /// <remarks>
    /// The present is the whole point of owning a swapchain, and it is also the thing PresentMon
    /// counts. Presenting an unchanged frame would inflate the very number §7 asks us to measure,
    /// which is why this is gated on <see cref="_dirty"/> rather than run unconditionally.
    /// </remarks>
    private void DrawFrame()
    {
        if (_swapChain is null || _text is null)
        {
            return;
        }

        var viewport = Viewport();
        if (viewport.Width <= 0f || viewport.Height <= 0f)
        {
            return;
        }

        _trace.FrameBegin();
        _text.ResetCounters();

        try
        {
            using (var ds = _swapChain.CreateDrawingSession(_theme.Surface))
            {
                DrawInto(ds, viewport);
            }

            // The present. This is the frame PresentMon counts, and the timestamp §7's budget is
            // measured against, which is precisely what a composition surface could not give us.
            _swapChain.Present();
        }
        catch (Exception ex) when (_swapChain is not null && _swapChain.Device.IsDeviceLost(ex.HResult))
        {
            // A GPU reset or a driver update. Every shaped layout and every cached page belongs to the
            // dead device, so they go with it; the next frame rebuilds against a fresh one.
            Log.Info("cal: graphics device lost, rebuilding the grid's resources");
            _panel.SwapChain = null;
            _swapChain.Dispose();
            _swapChain = null;
            _text.Dispose();
            _text = null;
            _pages.Clear();
            EnsureSwapChain();
        }

        _trace.FrameEnd(_text?.Shaped ?? 0);
    }

    private void DrawInto(CanvasDrawingSession ds, SurfaceViewport viewport)
    {
        if (_text is null)
        {
            return;
        }

        var m = _state.Metrics(viewport);

        // A recentre asked for before the control had a size lands here, on the first frame that has
        // one. Without it the grid opens at midnight on column zero, which reads as a scroll bug and
        // is really a lifecycle one.
        ApplyRecentre(m);

        // The recentre may have moved the strip across a seam, and a seam can change the banner's
        // height, so the geometry is re-read rather than reused. It is a handful of divides.
        m = _state.Metrics(Viewport());

        // **Clamp every frame, not only when a finger moved.** The vertical bound shifts with nobody
        // touching the glass: scroll to the bottom of a week whose banner is three lanes tall, scroll on
        // to a week with none, and the banner's rows go back to the grid, the grid grows taller,
        // MaxScrollY shrinks, and an offset nobody re-clamped is now past the end of the day, drawing a
        // strip of nothing below midnight. (Seen.) It costs two comparisons.
        _state.ClampScroll(m);

        // The width the labels are SHAPED against, frozen for the length of a pinch. The blocks still
        // scale every frame; it is the layout inside them that is held (§7).
        var shapeWidth = _state.ShapedDayWidth > 0f ? _state.ShapedDayWidth : m.DayWidth;

        var today = Today();
        BuildStrip(m, today);

        CalendarSurfaceDraw.DrawDays(
            ds,
            _strip,
            m,
            _theme,
            _strings,
            _text,
            scrollY: _state.ScrollY,
            contentTop: m.ContentTop,
            expanded: _state.BannerExpanded,
            nowMinutes: NowMinutes(),
            shapeWidth: shapeWidth,
            trace: _trace);

        // Last, and pinned: the days pass BEHIND the ruler, so it is drawn over them (§6).
        CalendarSurfaceDraw.DrawGutter(
            ds,
            m,
            _theme,
            _strings,
            _text,
            scrollY: _state.ScrollY,
            contentTop: m.ContentTop,
            weekNumber: Current.WeekNumber);

        TrimPages();
        UpdatePeriodTitle(m);
    }

    /// <summary>Which column of <paramref name="page"/> is today, or <c>-1</c>, on every other week.</summary>
    private static int IndexOfToday(PagePaint page, DateOnly today)
    {
        for (var i = 0; i < page.Headings.Count; i++)
        {
            if (page.Headings[i].Date == today)
            {
                return i;
            }
        }
        return -1;
    }

    /// <summary>The date <paramref name="index"/> days on from the anchor week's first day.</summary>
    /// <remarks>
    /// The strip in dates. <paramref name="index"/> may run past the end of the anchor week, that is
    /// what a seam <i>is</i>, so it carries into the next page rather than clamping to Sunday, which
    /// would name the period after a day that is not on screen.
    /// </remarks>
    private DateOnly? DateAt(int index)
    {
        if (index < 0)
        {
            return null;
        }
        var page = Page(_state.Week + (index / CalendarUnits.DaysInWeek));
        var column = index % CalendarUnits.DaysInWeek;
        return column < page.Headings.Count ? page.Headings[column].Date : null;
    }

    /// <summary>
    /// Names the period from the days actually <b>visible</b>, across a seam, if that is where the
    /// user is.
    /// </summary>
    /// <remarks>
    /// At the day zoom the user is looking at one column, and naming the month of a Sunday they cannot
    /// see is a small lie that reads as a bug. The same applies to a week that spans a month's end, and
    /// now to one that spans two weeks: the title follows the glass.
    /// </remarks>
    private void UpdatePeriodTitle(SurfaceMetrics m)
    {
        if (m.DayWidth <= 0f)
        {
            return;
        }
        var first = (int)MathF.Floor(_state.StripX / m.DayWidth);
        var last = (int)MathF.Floor((_state.StripX + m.DayViewport - 1f) / m.DayWidth);
        if (DateAt(first) is not { } from || DateAt(Math.Max(last, first)) is not { } to)
        {
            return;
        }

        var title = CalendarFormat.PeriodTitle(from, to, CultureInfo.CurrentCulture);
        if (title == PeriodTitle)
        {
            return;
        }
        PeriodTitle = title;
        PeriodChanged?.Invoke();
    }
}
