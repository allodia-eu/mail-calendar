// How much of an event block's label the grid draws at a zoom, decided without Win2D so
// Mailcal.Tests can drive it (docs/calendar.md §4, "An event block's label"). The twin of Apple's
// CalendarBlocks.swift, number for number, and of Linux's blocks.rs.
using System;
using System.Collections.Generic;

namespace Allodia.Mailcal.Calendar;

/// <summary>The rules behind an event block's label: which lines it draws, and where.</summary>
internal static class CalendarBlocks
{
    /// <summary>The bar of the calendar's colour down an event's leading edge, and the width of the
    /// hatch a hold draws in its place.</summary>
    internal const float BarWidth = 3f;

    /// <summary>Between a block's edge and the next block beside or below it.</summary>
    internal const float Gap = 1f;

    /// <summary>Between the bar and the label, and between the label and the trailing edge.</summary>
    internal const float LabelPadding = 4f;

    /// <summary>The narrowest a title may be squeezed to make room for the corner glyphs: a glyph is
    /// worth less than the name of the event.</summary>
    internal const float MinimumTitleWidth = 36f;

    /// <summary>Between two corner glyphs, and between the glyphs and the title.</summary>
    internal const float GlyphGap = 3f;

    /// <summary>The step a title's wrapping width is rounded down to, so a resize finds the layout it
    /// made a frame ago rather than shaping the text again (§7).</summary>
    internal const float LabelStep = 4f;

    /// <summary>The vertical inset of a block's label: a short block keeps every pixel it has.</summary>
    internal static float Inset(int minutes) => minutes < 30 ? Gap : Gap + 1f;

    /// <summary>The vertical space a <paramref name="minutes"/>-long block leaves for its label.</summary>
    internal static float LabelSpace(int minutes, float hourHeight) =>
        (hourHeight * minutes / CalendarUnits.MinutesInHour) - (Inset(minutes) * 2f);

    /// <summary>The line pitch a block's label is counted in.</summary>
    internal static float LineHeight(int minutes) => minutes < 30 ? 11f : 14f;

    /// <summary>The type size that goes with it.</summary>
    internal static float FontSize(int minutes) => minutes < 30 ? 9f : 11f;

    /// <summary>
    /// Whether a block is tall enough to hold its title <b>at this zoom</b>. One that is not stays a
    /// coloured block and keeps its spoken label, rather than drawing a title cut through the middle.
    /// </summary>
    internal static bool ShowsLabel(int minutes, float hourHeight) =>
        LabelSpace(minutes, hourHeight) >= LineHeight(minutes);

    /// <summary>Whether a block earns its time beneath the title: only once there is room for two lines.</summary>
    internal static bool ShowsTime(int minutes, float hourHeight) =>
        LabelSpace(minutes, hourHeight) >= LineHeight(minutes) * 2f;

    /// <summary>
    /// How many lines the title may wrap over: every whole line the block has room for, less the one
    /// its time takes when it draws one. Zero where there is no room for a label.
    /// </summary>
    internal static int TitleLines(int minutes, float hourHeight)
    {
        if (!ShowsLabel(minutes, hourHeight))
        {
            return 0;
        }
        var whole = (int)MathF.Floor(LabelSpace(minutes, hourHeight) / LineHeight(minutes));
        var time = ShowsTime(minutes, hourHeight) ? 1 : 0;
        return Math.Max(whole - time, 1);
    }

    /// <summary>The width a title is wrapped against: the room there is, rounded down to <see cref="LabelStep"/>.</summary>
    internal static float WrapWidth(float available) =>
        MathF.Floor(MathF.Max(available, 0f) / LabelStep) * LabelStep;

    /// <summary>
    /// Whether the title wraps by word: only where its widest line, wrapped at whole words, fits the
    /// width. A word wider than the block would be broken across two lines, which reads as a broken
    /// grid, so the title then runs on as one line and is cut at the block's edge.
    /// </summary>
    internal static bool Wraps(float widestLine, float wrapWidth) => widestLine <= wrapWidth + 0.5f;

    /// <summary>The width the meeting and repeat glyphs take side by side, each <paramref name="size"/> square.</summary>
    internal static float GlyphsWidth(int count, float size) =>
        count <= 0 ? 0f : (count * size) + ((count - 1) * GlyphGap);

    /// <summary>
    /// The room the title keeps beside the corner glyphs, or <c>null</c> where drawing them would leave
    /// it less than <see cref="MinimumTitleWidth"/>: the glyphs then give way and the title has it all.
    /// </summary>
    internal static float? BesideGlyphs(float width, float glyphsWidth)
    {
        if (glyphsWidth <= 0f)
        {
            return null;
        }
        var room = width - glyphsWidth - GlyphGap;
        return room >= MinimumTitleWidth ? room : null;
    }

    /// <summary>Whether the time line draws the range (09:00–12:00) or, where the column is too
    /// narrow for it, the start alone.</summary>
    internal static bool ShowsRange(float rangeWidth, float width) => rangeWidth <= width;

    /// <summary>
    /// The height of the most whole lines, at most <paramref name="lines"/> and never fewer than one,
    /// that end within <paramref name="room"/>. <paramref name="lineBottoms"/> is where each line of
    /// the wrapped title ends, from its top. The pitch a block counts its lines in is nominal, and a
    /// typeface with taller lines must not push the last line, or the time beneath it, through the
    /// block's edge.
    /// </summary>
    internal static float Fitting(IReadOnlyList<float> lineBottoms, int lines, float room)
    {
        if (lineBottoms.Count == 0)
        {
            return 0f;
        }
        var most = Math.Min(Math.Max(lines, 1), lineBottoms.Count);
        var height = lineBottoms[0];
        for (var i = 1; i < most && lineBottoms[i] <= room; i++)
        {
            height = lineBottoms[i];
        }
        return height;
    }
}
