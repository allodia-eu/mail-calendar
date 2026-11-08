// An event on the drawn grid: its shape in the calendar's colour (docs/calendar.md §1) and its label
// (§4). A tint fills it and the colour itself runs down its leading edge; an unanswered hold swaps
// that bar for a hatched edge and a dashed border (CalendarHold). A block's title wraps over every
// whole line it has room for, the time beneath it, the meeting and repeat glyphs in its top-right
// corner; an all-day bar's title runs on one line with the camera after it. Which lines a block draws
// is CalendarBlocks', so Mailcal.Tests can drive it.
using System;
using Microsoft.Graphics.Canvas;
using Windows.Foundation;
using Windows.UI;

namespace Allodia.Mailcal.Calendar;

/// <summary>Draws one event's shape and label on the canvas.</summary>
internal static class CalendarBlockDraw
{
    /// <summary>Segoe Fluent Icons "RepeatAll": the mark on an occurrence of a repeating event.</summary>
    internal const string RepeatGlyph = "";

    /// <summary>
    /// An event's shape in <paramref name="rect"/>: the tint, then the calendar's colour down the
    /// leading edge, or for a hold the hatched edge and the dashed border instead.
    /// </summary>
    internal static void Shape(CanvasDrawingSession ds, Rect rect, EventColors colors, bool awaiting)
    {
        if (rect.Width <= 0 || rect.Height <= 0)
        {
            return;
        }
        var corner = MathF.Min(SurfaceTheme.CornerRadius, (float)Math.Min(rect.Width, rect.Height) / 2f);
        ds.FillRoundedRectangle(rect, corner, corner, colors.Fill);
        if (awaiting)
        {
            CalendarHold.Draw(ds, rect, colors.Accent, corner, awaiting: true);
            return;
        }
        // The block's own rounded shape, cut to the bar's width, so the bar follows the corners.
        var bar = Math.Min(CalendarBlocks.BarWidth, rect.Width);
        using var layer = ds.CreateLayer(1f, new Rect(rect.X, rect.Y, bar, rect.Height));
        ds.FillRoundedRectangle(rect, corner, corner, colors.Accent);
    }

    /// <summary>
    /// A timed block's label in <paramref name="rect"/>, drawn at <paramref name="hourHeight"/>. A block
    /// too short for one whole line draws none: a title cut through the middle reads as a broken grid.
    /// </summary>
    /// <param name="shapedRoom">The room the label is laid out in, from the width the labels are
    /// shaped against, which during a pinch is frozen while <paramref name="rect"/> tracks the fingers.</param>
    internal static void BlockLabel(
        CanvasDrawingSession ds,
        TextLayoutCache text,
        SurfaceTheme theme,
        BlockPaint block,
        Rect rect,
        float hourHeight,
        float shapedRoom)
    {
        var minutes = block.Minutes;
        var lines = CalendarBlocks.TitleLines(minutes, hourHeight);
        if (lines == 0)
        {
            return;
        }
        var size = CalendarBlocks.FontSize(minutes);
        var space = CalendarBlocks.LabelSpace(minutes, hourHeight);
        var withTime = CalendarBlocks.ShowsTime(minutes, hourHeight);
        var top = (float)rect.Y + CalendarBlocks.Inset(minutes) - CalendarBlocks.Gap;
        var left = (float)rect.X + CalendarBlocks.BarWidth + CalendarBlocks.LabelPadding;
        var width = MathF.Max((float)rect.Right - CalendarBlocks.LabelPadding - left, 0f);
        var room = MathF.Max(shapedRoom, 0f);
        var colour = block.Colors.Text;
        var titleFormat = theme.TitleFormat(minutes);
        var title = text.Run(block.Title, titleFormat);

        using var clip = ds.CreateLayer(1f, rect);

        // In the top-right corner, where Apple's Calendar keeps its own, so the title wraps beside
        // them; and only while the title keeps room of its own.
        var count = (block.Meeting ? 1 : 0) + (block.Repeats ? 1 : 0);
        var glyphsWidth = CalendarBlocks.GlyphsWidth(count, size);
        var available = room;
        // What the title is cut to: the room it was laid out in, or less mid-pinch, where the live
        // block is narrower than the frozen width and the glyphs follow its edge.
        var visible = width;
        if (CalendarBlocks.BesideGlyphs(room, glyphsWidth) is { } beside)
        {
            available = beside;
            visible = width - glyphsWidth - CalendarBlocks.GlyphGap;
            var x = left + width - glyphsWidth;
            var glyphFormat = theme.GlyphFormat(minutes);
            if (block.Meeting)
            {
                Glyph(ds, text, EventMeeting.CameraGlyph, glyphFormat, x, top, title.Height, colour);
                x += size + CalendarBlocks.GlyphGap;
            }
            if (block.Repeats)
            {
                Glyph(ds, text, RepeatGlyph, glyphFormat, x, top, title.Height, colour);
            }
        }

        ShapedText? time = null;
        ShapedText? range = null;
        var timeHeight = 0f;
        if (withTime)
        {
            var timeFormat = theme.TimeFormat(minutes);
            time = text.Run(block.Time, timeFormat);
            range = text.Run(block.Range, timeFormat);
            timeHeight = MathF.Max(time.Height, range.Height);
        }

        visible = MathF.Max(MathF.Min(visible, available), 0f);
        var wrap = CalendarBlocks.WrapWidth(available);
        var wrapped = wrap > 0f ? text.Wrapped(block.Title, titleFormat, wrap) : null;
        float titleHeight;
        if (wrapped is not null && CalendarBlocks.Wraps(wrapped.Width, wrap))
        {
            // Cut after the last whole line there is room for, where DirectWrite ended that line, so
            // nothing of the next line shows beneath it.
            titleHeight = CalendarBlocks.Fitting(wrapped.LineBottoms, lines, space - timeHeight);
            using var cut = ds.CreateLayer(1f, new Rect(left, top, visible, titleHeight));
            ds.DrawTextLayout(wrapped.Layout, left, top, colour);
        }
        else
        {
            // A word wider than the block would be broken across two lines, so the title runs on as
            // one line and is cut at the block's edge instead.
            titleHeight = title.Height;
            using var cut = ds.CreateLayer(1f, new Rect(left, top, visible, titleHeight));
            ds.DrawTextLayout(title.Layout, left, top, colour);
        }

        // Only where it ends inside the block: a time cut through the middle reads as a broken grid.
        if (time is null || range is null || titleHeight + timeHeight > space)
        {
            return;
        }
        var shown = CalendarBlocks.ShowsRange(range.Width, room) ? range : time;
        var y = top + titleHeight;
        using var below = ds.CreateLayer(1f, new Rect(left, y, width, shown.Height));
        ds.DrawTextLayout(shown.Layout, left, y, colour);
    }

    /// <summary>
    /// An all-day bar's title on one line, and the camera after it for an event with a video meeting.
    /// The title gives up the camera's room first, so the camera stays in sight; a bar too narrow for
    /// both draws the title alone, and its spoken label still says "video meeting".
    /// </summary>
    internal static void BandLabel(
        CanvasDrawingSession ds, TextLayoutCache text, SurfaceTheme theme, BandPaint band, Rect rect)
    {
        var left = (float)rect.X + CalendarBlocks.BarWidth + CalendarBlocks.LabelPadding;
        var room = MathF.Max((float)rect.Right - CalendarBlocks.LabelPadding - left, 0f);
        if (room <= 0f)
        {
            return;
        }
        var reserve = SurfaceTheme.BandFontSize + CalendarBlocks.GlyphGap;
        var camera = band.Meeting && room - reserve >= CalendarBlocks.MinimumTitleWidth;
        var titleRoom = camera ? room - reserve : room;
        var title = text.Run(band.Title, theme.TitleLarge);
        var top = (float)rect.Y + MathF.Max(((float)rect.Height - title.Height) / 2f, 0f);
        using (ds.CreateLayer(1f, new Rect(left, rect.Y, titleRoom, rect.Height)))
        {
            ds.DrawTextLayout(title.Layout, left, top, band.Colors.Text);
        }
        if (camera)
        {
            var x = left + MathF.Min(title.Width, titleRoom) + CalendarBlocks.GlyphGap;
            using var clip = ds.CreateLayer(1f, rect);
            Glyph(ds, text, EventMeeting.CameraGlyph, theme.GlyphLarge, x, top, title.Height, band.Colors.Text);
        }
    }

    /// <summary>A corner glyph at <paramref name="x"/>, centred on the title line that starts at
    /// <paramref name="top"/>.</summary>
    private static void Glyph(
        CanvasDrawingSession ds,
        TextLayoutCache text,
        string glyph,
        Microsoft.Graphics.Canvas.Text.CanvasTextFormat format,
        float x,
        float top,
        float line,
        Color colour)
    {
        var shaped = text.Run(glyph, format);
        ds.DrawTextLayout(shaped.Layout, x, top + ((line - shaped.Height) / 2f), colour);
    }
}
