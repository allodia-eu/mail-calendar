// An event's title on a block or bar, with the camera after it when the event has a video meeting
// (docs/conferencing.md, rule 7). The title truncates first, so the camera stays in sight; a block
// too narrow for both draws the title alone, and its spoken label still says "video meeting".
using System;
using Microsoft.Graphics.Canvas;
using Microsoft.Graphics.Canvas.Text;
using Windows.UI;

namespace Allodia.Mailcal.Calendar;

/// <summary>Draws one title line of the grid, and the camera that may follow it.</summary>
internal static class CalendarTitleDraw
{
    /// <summary>Below this much room for the title itself, the camera gives way to the title.</summary>
    private const float MinTitleRoom = 16f;

    /// <summary>
    /// The width the camera is shaped against: wider than the glyph, so the cache's bucketing never
    /// trims it to an ellipsis. The block's clip bounds what is drawn.
    /// </summary>
    private const float GlyphShapeWidth = 32f;

    /// <summary>Format ids for the cache, beside the title formats' 0 (small) and 1 (large).</summary>
    private const int GlyphFormatOffset = 2;

    /// <summary>
    /// Draws <paramref name="title"/> at (<paramref name="x"/>, <paramref name="y"/>) inside
    /// <paramref name="room"/>, and the camera after it when <paramref name="meeting"/> is set.
    /// </summary>
    internal static void Draw(
        CanvasDrawingSession ds,
        TextLayoutCache text,
        SurfaceTheme theme,
        string title,
        bool meeting,
        CanvasTextFormat format,
        int formatId,
        float room,
        float x,
        float y,
        Color colour)
    {
        var reserve = format.FontSize + SurfaceTheme.GlyphGap;
        var withCamera = meeting && room - reserve >= MinTitleRoom;
        var titleRoom = withCamera ? room - reserve : room;

        var layout = text.Line(title, format, titleRoom, formatId);
        ds.DrawTextLayout(layout, x, y, colour);
        if (!withCamera)
        {
            return;
        }

        // Directly after the text as drawn, which an ellipsis has already shortened to fit.
        var after = MathF.Min((float)layout.LayoutBounds.Width, titleRoom);
        // The title's formatId names its size (0 small, 1 large), so the glyph's format and its
        // cache key are read off the same value and cannot disagree.
        var glyph = formatId == 0 ? theme.GlyphSmall : theme.GlyphLarge;
        var camera = text.Line(EventMeeting.CameraGlyph, glyph, GlyphShapeWidth, formatId + GlyphFormatOffset);
        ds.DrawTextLayout(camera, x + after + SurfaceTheme.GlyphGap, y + 1f, colour);
    }
}
