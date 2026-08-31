// The grid is DRAWN, not composed.
//
// §7 of the calendar contract, and it is not a style preference. Android built this out of one
// composable per event and paid a layout pass per block on every frame of a pinch; the grid's own
// draw is trivial by comparison, it is rectangles and short strings. Measured on the same phone,
// against the same 1,100-occurrence diary: the composable grid dropped 20.7% of in-motion frames,
// the canvas 7.4%, and the canvas *delivered a third more frames*, which is the same fact said twice.
//
// What a frame is allowed to do: multiply the core's unit-free geometry by an hour height and a
// column width, and cull whatever falls outside the viewport. Everything else, the colours, the
// clocks, the spoken labels, the shaped text, was decided before the frame began (CalendarPaint,
// SurfaceTheme, TextLayoutCache).
//
// **The hour ruler is drawn ONCE, and it does not move.**
//
// It used to belong to the week. Each page drew its own, its own gutter, its own "wk 30", its own
// 00:00–23:00, and the page slid sideways with all of it, because a page was a week and the ruler was
// part of it. That is invisible while a week is *turning*, and it is why the grid could only ever come
// to rest on a week boundary: stop halfway and a second column of hour labels is parked in the middle
// of the screen, with no ruler at all down the left-hand edge. (Screenshot in the PR. It is exactly as
// bad as it sounds.)
//
// So the ruler is chrome now, not content: it is pinned to the left edge, the days scroll past it, and
// the weeks are laid end to end as one strip with no gutter between them. Which is what lets a
// touchpad pan come to rest wherever the user stopped pushing, the thing this whole change is for
// (§6), and it is what macOS Calendar does, for the same reason.
//
// One thing the pinned ruler *demands* in return: a single content top. The grid's 00:00 is where the
// ruler's 00:00 is, so every week on screen must give its all-day banner the same height, even where
// one of them has three lanes and its neighbour has none. That is why SurfaceViewport.Lanes is the max
// across the visible weeks and not the current page's own.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.Numerics;
using Microsoft.Graphics.Canvas;
using Windows.Foundation;

namespace Allodia.Mailcal.Calendar;

/// <summary>
/// One week of the strip, and where its first day sits.
/// </summary>
/// <remarks>
/// <paramref name="Left"/> is in <b>day-viewport</b> coordinates: <c>0</c> is the first pixel to the
/// right of the pinned hour ruler. It is negative for the week the strip has partly scrolled off, which
/// is the ordinary resting state now and not a gesture in progress.
/// </remarks>
internal readonly record struct StripPage(PagePaint Page, float Left, int TodayIndex);

/// <summary>Draws the strip of weeks, and the chrome that does not move with it.</summary>
internal static class CalendarSurfaceDraw
{
    /// <summary>The now line, and the today underline.</summary>
    private const float NowLineThickness = 2f;

    /// <summary>
    /// The days: headings, all-day bands, and the grid, every visible week, in one pass per band.
    /// </summary>
    /// <remarks>
    /// One clip per horizontal band rather than one per page, because the bands are the things with
    /// edges: the strip runs continuously through them, and a week is just an offset along it.
    /// <para>
    /// <paramref name="shapeWidth"/> is the width labels are <b>shaped</b> against, which during a
    /// pinch is deliberately not the width they are <b>drawn</b> at. See
    /// <see cref="CalendarSurfaceState.ShapedDayWidth"/>, the rectangle tracks the fingers, the
    /// shaper sleeps.
    /// </para>
    /// </remarks>
    internal static void DrawDays(
        CanvasDrawingSession ds,
        IReadOnlyList<StripPage> pages,
        SurfaceMetrics m,
        SurfaceTheme theme,
        SurfaceStrings strings,
        TextLayoutCache text,
        float scrollY,
        float contentTop,
        bool expanded,
        int nowMinutes,
        float shapeWidth,
        CalendarTrace trace)
    {
        DrawHeadings(ds, pages, m, theme);
        if (m.BannerLanes > 0)
        {
            DrawBands(ds, pages, m, theme, text, contentTop, expanded, shapeWidth);
        }
        DrawGrid(ds, pages, m, theme, text, scrollY, contentTop, nowMinutes, shapeWidth, trace);
        DrawLoading(ds, pages, m, theme, strings, text, contentTop);
    }

    /// <summary>
    /// The chrome the days scroll <b>past</b>: the hour ruler, the week number, the "all day" label.
    /// </summary>
    /// <remarks>
    /// Drawn once, in surface coordinates, after the days, so a block that has scrolled under the
    /// ruler is covered by it rather than drawn over it.
    /// <para>
    /// <paramref name="weekNumber"/> is the week of the <b>leftmost day on screen</b>, which is the
    /// anchor week by construction (the strip is always inside it). Scroll across a seam and the number
    /// changes when the day at the left edge does, which is the only answer that is true of what is
    /// actually under it.
    /// </para>
    /// </remarks>
    internal static void DrawGutter(
        CanvasDrawingSession ds,
        SurfaceMetrics m,
        SurfaceTheme theme,
        SurfaceStrings strings,
        TextLayoutCache text,
        float scrollY,
        float contentTop,
        int weekNumber)
    {
        // The ruler's own column, opaque: the days pass behind it, not through it.
        ds.FillRectangle(new Rect(0, 0, m.Gutter, m.Height), theme.Surface);

        if (weekNumber > 0)
        {
            ds.DrawText(
                strings.WeekShort,
                new Rect(0, 8, MathF.Max(m.Gutter - 8f, 0f), 14),
                theme.Muted,
                theme.Hour);
            ds.DrawText(
                weekNumber.ToString(CultureInfo.CurrentCulture),
                new Rect(0, 22, MathF.Max(m.Gutter - 8f, 0f), 22),
                theme.Text,
                theme.Weekday);
        }

        if (m.BannerLanes > 0)
        {
            ds.DrawText(
                strings.AllDay,
                new Rect(4, m.Viewport.HeaderHeight + 2, MathF.Max(m.Gutter - 8f, 0f), m.Viewport.LaneHeight),
                theme.Muted,
                theme.Hour);
        }

        // The line under the header and banner, across the whole surface, the grid's top edge, and
        // the one the ruler's hours are measured from.
        ds.DrawLine(0f, contentTop, m.Width, contentTop, theme.Line, m.Viewport.DividerHeight);

        var clip = new Rect(0, contentTop, m.Gutter, MathF.Max(m.Height - contentTop, 0f));
        using var layer = ds.CreateLayer(1f, clip);

        var origin = ds.Transform;
        ds.Transform = Matrix3x2.CreateTranslation(0f, contentTop - scrollY) * origin;

        for (var h = 0; h < CalendarUnits.HoursInDay; h++)
        {
            var label = strings.Hours[h];
            if (label.Length == 0)
            {
                continue;
            }
            var y = m.HourHeight * h;
            if (y < scrollY - m.HourHeight || y > scrollY + m.ContentHeight)
            {
                continue;
            }
            ds.DrawText(label, new Rect(0, y - 7, MathF.Max(m.Gutter - 8f, 0f), 16), theme.Muted, theme.Hour);
        }

        ds.Transform = origin;
    }

    private static void DrawHeadings(
        CanvasDrawingSession ds,
        IReadOnlyList<StripPage> pages,
        SurfaceMetrics m,
        SurfaceTheme theme)
    {
        var clip = new Rect(m.Gutter, 0, m.DayViewport, m.Viewport.HeaderHeight);
        using var layer = ds.CreateLayer(1f, clip);

        var origin = ds.Transform;
        ds.Transform = Matrix3x2.CreateTranslation(m.Gutter, 0f) * origin;

        foreach (var strip in pages)
        {
            for (var i = 0; i < strip.Page.Headings.Count; i++)
            {
                var left = strip.Left + (m.DayWidth * i);
                // Cull the columns scrolled off the strip. At the day zoom that is six of the seven.
                if (left + m.DayWidth <= 0f || left >= m.DayViewport)
                {
                    continue;
                }

                var heading = strip.Page.Headings[i];
                var isToday = i == strip.TodayIndex;
                var colour = isToday ? theme.Today : theme.Muted;

                ds.DrawText(heading.Weekday, new Rect(left, 6, m.DayWidth, 16), colour, theme.Weekday);
                ds.DrawText(
                    heading.DayOfMonth,
                    new Rect(left, 20, m.DayWidth, 26),
                    isToday ? theme.Today : theme.Text,
                    theme.DayNumber);
            }
        }

        ds.Transform = origin;
    }

    /// <summary>
    /// The all-day bands, and the per-day "+N" chips.
    /// </summary>
    /// <remarks>
    /// The banner's <b>height</b> is the strip's (one content top, §6), but its <b>lanes</b> are each
    /// week's own: a quiet week beside a busy one draws one row and leaves the rest of the band empty,
    /// rather than stretching its bars to fill a height that is not its.
    /// </remarks>
    private static void DrawBands(
        CanvasDrawingSession ds,
        IReadOnlyList<StripPage> pages,
        SurfaceMetrics m,
        SurfaceTheme theme,
        TextLayoutCache text,
        float contentTop,
        bool expanded,
        float shapeWidth)
    {
        var top = m.Viewport.HeaderHeight;
        var clip = new Rect(m.Gutter, top, m.DayViewport, MathF.Max(contentTop - top, 0f));
        using var layer = ds.CreateLayer(1f, clip);

        var origin = ds.Transform;
        ds.Transform = Matrix3x2.CreateTranslation(m.Gutter, top) * origin;

        foreach (var strip in pages)
        {
            var page = strip.Page;
            var drawnLanes = CalendarAllDay.DrawnLanes(page.Lanes, expanded);

            foreach (var band in page.Bands)
            {
                if (band.Span.Lane >= drawnLanes)
                {
                    continue;
                }
                var r = m.BandRect(band.Span).Translate(strip.Left, 0f);
                var rect = new Rect(
                    r.Left + SurfaceTheme.BlockGap,
                    r.Top + SurfaceTheme.BlockGap,
                    MathF.Max(r.Width - (SurfaceTheme.BlockGap * 2f), 0f),
                    MathF.Max(r.Height - (SurfaceTheme.BlockGap * 2f), 0f));
                if (rect.Right <= 0 || rect.X >= m.DayViewport || rect.Width <= 0)
                {
                    continue;
                }
                ds.FillRoundedRectangle(rect, SurfaceTheme.CornerRadius, SurfaceTheme.CornerRadius, band.Background);
                // A bar draws no border of its own, so an unanswered hold gains the whole treatment
                // in one call. A no-op on a commitment (CalendarHold).
                CalendarHold.Draw(ds, rect, band.Border, SurfaceTheme.CornerRadius, band.Awaiting);

                var room = (float)rect.Width - (SurfaceTheme.BlockPadding * 2f);
                if (room <= 0f)
                {
                    continue;
                }
                using var titleLayer = ds.CreateLayer(1f, rect);
                var layout = text.Line(band.Title, theme.BlockLarge, room, formatId: 1);
                ds.DrawTextLayout(
                    layout,
                    (float)rect.X + SurfaceTheme.BlockPadding,
                    (float)rect.Y + 2f,
                    band.Text);
            }

            // The "+N" chips, one per column, because a hidden multi-day bar is hidden on every day it
            // covers, and a single global "+N" would be wrong on every column but one (§4).
            if (drawnLanes >= page.Lanes)
            {
                continue;
            }
            for (var day = 0; day < page.MoreLabels.Count; day++)
            {
                var label = page.MoreLabels[day];
                if (label.Length == 0)
                {
                    continue;
                }
                var r = m.MoreRect(day, drawnLanes).Translate(strip.Left, 0f);
                if (r.Right <= 0f || r.Left >= m.DayViewport)
                {
                    continue;
                }
                ds.DrawText(
                    label,
                    new Rect(r.Left + SurfaceTheme.BlockPadding, r.Top + 3f, MathF.Max(r.Width - 4f, 0f), r.Height),
                    theme.Muted,
                    theme.Chrome);
            }
        }

        ds.Transform = origin;
    }

    private static void DrawGrid(
        CanvasDrawingSession ds,
        IReadOnlyList<StripPage> pages,
        SurfaceMetrics m,
        SurfaceTheme theme,
        TextLayoutCache text,
        float scrollY,
        float contentTop,
        int nowMinutes,
        float shapeWidth,
        CalendarTrace trace)
    {
        var clip = new Rect(
            m.Gutter,
            contentTop,
            m.DayViewport,
            MathF.Max(m.Height - contentTop, 0f));
        using var layer = ds.CreateLayer(1f, clip);

        var origin = ds.Transform;
        ds.Transform = Matrix3x2.CreateTranslation(m.Gutter, contentTop - scrollY) * origin;

        // The viewport, in content coordinates, what the cull tests against.
        var top = scrollY;
        var bottom = scrollY + MathF.Max(m.Height - contentTop, 0f);

        foreach (var strip in pages)
        {
            DrawGridLines(ds, strip, m, theme, top, bottom);

            foreach (var block in strip.Page.Blocks)
            {
                var r = m.BlockRect(block.Span).Translate(strip.Left, 0f);
                if (r.Bottom <= top || r.Top >= bottom || r.Right <= 0f || r.Left >= m.DayViewport)
                {
                    trace.Culled();
                    continue;
                }
                trace.Drew();
                DrawBlock(ds, block, r, m, theme, text, shapeWidth);
            }

            if (strip.TodayIndex >= 0)
            {
                var y = m.HourHeight * (nowMinutes / CalendarUnits.MinutesInHour);
                var x = strip.Left + (m.DayWidth * strip.TodayIndex);
                ds.DrawLine(x, y, x + m.DayWidth, y, theme.Now, NowLineThickness);
                ds.FillCircle(x + 3f, y, 3f, theme.Now);
            }
        }

        ds.Transform = origin;
    }

    private static void DrawGridLines(
        CanvasDrawingSession ds,
        StripPage strip,
        SurfaceMetrics m,
        SurfaceTheme theme,
        float top,
        float bottom)
    {
        // Cheaper to draw than to cull: 24 lines and 7 columns is nothing next to a layout pass. The
        // hour lines span this week only, and meet the next week's exactly, the strip has no seams in
        // it, so neither may they.
        for (var h = 0; h <= CalendarUnits.HoursInDay; h++)
        {
            var y = m.HourHeight * h;
            if (y < top - 1f || y > bottom + 1f)
            {
                continue;
            }
            ds.DrawLine(strip.Left, y, strip.Left + m.WeekWidth, y, theme.HourLine, 1f);
        }
        for (var d = 0; d <= strip.Page.Headings.Count; d++)
        {
            var x = strip.Left + (m.DayWidth * d);
            ds.DrawLine(x, 0f, x, m.GridHeight, theme.Line, 1f);
        }
    }

    private static void DrawBlock(
        CanvasDrawingSession ds,
        BlockPaint block,
        GridRect r,
        SurfaceMetrics m,
        SurfaceTheme theme,
        TextLayoutCache text,
        float shapeWidth)
    {
        var inset = SurfaceTheme.BlockInset(block.Minutes);
        var rect = new Rect(
            r.Left + SurfaceTheme.BlockGap,
            r.Top + SurfaceTheme.BlockGap,
            MathF.Max(r.Width - (SurfaceTheme.BlockGap * 2f), 0f),
            MathF.Max(r.Height - (SurfaceTheme.BlockGap * 2f), 0f));
        if (rect.Width <= 0 || rect.Height <= 0)
        {
            return;
        }

        ds.FillRoundedRectangle(rect, SurfaceTheme.CornerRadius, SurfaceTheme.CornerRadius, block.Background);
        // A block already has a hairline, so a hold restyles it rather than gaining a second edge.
        ds.DrawRoundedRectangle(
            rect, SurfaceTheme.CornerRadius, SurfaceTheme.CornerRadius, block.Border, 1f,
            CalendarHold.Stroke(block.Awaiting));
        CalendarHold.Hatch(ds, rect, block.Border, block.Awaiting);

        if (!SurfaceTheme.ShowsLabel(block.Minutes, m.HourHeight))
        {
            // Too short to hold text at this zoom. It stays a coloured block and KEEPS ITS FULL SPOKEN
            // LABEL (§4), the semantics overlay reads block.Spoken regardless of what is drawn.
            return;
        }

        // Shaped against the FROZEN width, drawn and clipped against the LIVE one. Mid-pinch those
        // differ, and that is the entire point: the rectangle tracks the fingers, the shaper sleeps.
        var room = (shapeWidth / block.Span.Columns) - (inset * 2f) - (SurfaceTheme.BlockGap * 2f);
        if (room <= 0f)
        {
            return;
        }

        using var clip = ds.CreateLayer(1f, rect);
        var format = theme.BlockFormat(block.Minutes);
        var formatId = block.Minutes < 30 ? 0 : 1;
        var y = (float)rect.Y + inset;

        var title = text.Line(block.Title, format, room, formatId);
        ds.DrawTextLayout(title, (float)rect.X + inset, y, block.Text);

        if (SurfaceTheme.ShowsTime(block.Minutes, m.HourHeight))
        {
            y += SurfaceTheme.BlockLineHeight(block.Minutes);
            var clock = text.Line(block.Clock, format, room, formatId);
            ds.DrawTextLayout(clock, (float)rect.X + inset, y, block.Text);
        }
    }

    /// <summary>
    /// A week the core has not answered for, saying so, over its own columns, and no further.
    /// </summary>
    /// <remarks>
    /// <c>IsMaterialized == false</c> <b>does not mean "no events"</b> (§4). A confidently empty week is
    /// a lie that looks exactly like a real answer. It is drawn per week, because at a seam one week may
    /// be loaded and its neighbour not, and a bar across the whole surface would libel the loaded one.
    /// </remarks>
    private static void DrawLoading(
        CanvasDrawingSession ds,
        IReadOnlyList<StripPage> pages,
        SurfaceMetrics m,
        SurfaceTheme theme,
        SurfaceStrings strings,
        TextLayoutCache text,
        float contentTop)
    {
        var height = m.Viewport.LaneHeight;
        var clip = new Rect(m.Gutter, contentTop, m.DayViewport, MathF.Min(height, MathF.Max(m.Height - contentTop, 0f)));
        using var layer = ds.CreateLayer(1f, clip);

        var origin = ds.Transform;
        ds.Transform = Matrix3x2.CreateTranslation(m.Gutter, 0f) * origin;

        foreach (var strip in pages)
        {
            if (strip.Page.IsMaterialized || strip.Page.Headings.Count == 0)
            {
                continue;
            }
            var rect = new Rect(strip.Left, contentTop, m.WeekWidth, height);
            ds.FillRectangle(rect, theme.Surface);
            ds.DrawText(
                strings.Loading,
                new Rect(strip.Left + 8f, contentTop + 3f, MathF.Max(m.WeekWidth - 16f, 0f), height),
                theme.Muted,
                theme.Chrome);
            ds.DrawLine(
                strip.Left,
                contentTop + height,
                strip.Left + m.WeekWidth,
                contentTop + height,
                theme.Now,
                NowLineThickness);
        }

        ds.Transform = origin;
    }
}
