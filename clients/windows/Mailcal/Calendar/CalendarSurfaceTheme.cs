// The renderer's palette, its type, and the one cache that decides whether a pinch holds 60fps.
//
// A canvas has no styles and no resources: a draw call gets colours and text formats or it derives
// them itself, sixty times a second, per block. So they are built once, here, and handed to the
// frame, which is §7's "nothing that a zoom cannot change may be re-derived on a zoom", made
// structural.
using System;
using System.Collections.Generic;
using System.Globalization;
using Microsoft.Graphics.Canvas;
using Microsoft.Graphics.Canvas.Text;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Windows.UI;

namespace Allodia.Mailcal.Calendar;

/// <summary>Everything the renderer draws with that is not a page: the theme's colours and its type.</summary>
internal sealed class SurfaceTheme : IDisposable
{
    /// <summary>The hour ruler's width.</summary>
    internal const float Gutter = 56f;

    /// <summary>The height of one all-day lane.</summary>
    internal const float LaneHeight = 24f;

    /// <summary>The day-heading strip.</summary>
    internal const float HeaderHeight = 52f;

    /// <summary>The seam between the banner and the grid.</summary>
    internal const float DividerHeight = 1f;

    /// <summary>A block's rounded corner.</summary>
    internal const float CornerRadius = 4f;

    /// <summary>The gap that keeps two adjacent blocks from touching.</summary>
    internal const float BlockGap = 1f;

    /// <summary>The padding inside the coloured chip. A short block spends nothing on it, every
    /// pixel is the difference between showing the title and not.</summary>
    internal const float BlockPadding = 2f;

    internal SurfaceTheme(bool dark)
    {
        Dark = dark;
        Line = dark ? Color.FromArgb(255, 60, 60, 64) : Color.FromArgb(255, 224, 224, 228);
        HourLine = dark ? Color.FromArgb(255, 48, 48, 52) : Color.FromArgb(255, 236, 236, 240);
        Text = dark ? Color.FromArgb(255, 230, 230, 235) : Color.FromArgb(255, 28, 28, 30);
        Muted = dark ? Color.FromArgb(255, 150, 150, 158) : Color.FromArgb(255, 110, 110, 118);
        Surface = dark ? Color.FromArgb(255, 24, 24, 27) : Colors.White;

        // Allodia Orange means "action", which is exactly what "now" is. It is deliberately absent
        // from the calendar palette, so nothing can collide with it.
        Now = Color.FromArgb(255, 232, 106, 51);
        Today = Now;

        Weekday = Format(12f, CanvasHorizontalAlignment.Center);
        DayNumber = Format(19f, CanvasHorizontalAlignment.Center);
        Hour = Format(11f, CanvasHorizontalAlignment.Right);
        Chrome = Format(12f, CanvasHorizontalAlignment.Left);

        // Two rungs. A quarter-hour block at the default horizon is ~11px tall, and a 16px line box
        // slices its title through the middle, geometrically perfect, visibly broken.
        BlockSmall = Format(9f, CanvasHorizontalAlignment.Left);
        BlockLarge = Format(11f, CanvasHorizontalAlignment.Left);
    }

    internal bool Dark { get; }

    internal Color Line { get; }

    internal Color HourLine { get; }

    internal Color Text { get; }

    internal Color Muted { get; }

    internal Color Surface { get; }

    internal Color Now { get; }

    internal Color Today { get; }

    internal CanvasTextFormat Weekday { get; }

    internal CanvasTextFormat DayNumber { get; }

    internal CanvasTextFormat Hour { get; }

    internal CanvasTextFormat Chrome { get; }

    internal CanvasTextFormat BlockSmall { get; }

    internal CanvasTextFormat BlockLarge { get; }

    /// <summary>The theme the app is actually showing, read once out of XAML.</summary>
    internal static SurfaceTheme For(ElementTheme theme, bool appIsDark) =>
        new(theme == ElementTheme.Dark || (theme == ElementTheme.Default && appIsDark));

    /// <summary>The line box a block of <paramref name="minutes"/> gets.</summary>
    internal static float BlockLineHeight(int minutes) => minutes < 30 ? 11f : 15f;

    /// <summary>The inset a block of <paramref name="minutes"/> gets.</summary>
    internal static float BlockInset(int minutes) =>
        minutes < 30 ? BlockGap : BlockGap + BlockPadding;

    /// <summary>The vertical room a block leaves for its label, at this zoom.</summary>
    internal static float BlockLabelSpace(int minutes, float hourHeight) =>
        (hourHeight * (minutes / CalendarUnits.MinutesInHour)) - (BlockInset(minutes) * 2f);

    /// <summary>
    /// Whether a block is tall enough to hold its own title <b>at this zoom</b>.
    /// </summary>
    /// <remarks>
    /// Zoomed out to the whole day, a 15-minute event is a few pixels tall and <i>cannot</i> hold
    /// text, so it doesn't get any, rather than getting a title cut through the middle. It stays a
    /// coloured block, keeps its full spoken label for a screen reader, and reveals its title when the
    /// user zooms in. This is why the rule has to be a function of the zoom rather than a constant
    /// (§4).
    /// </remarks>
    internal static bool ShowsLabel(int minutes, float hourHeight) =>
        BlockLabelSpace(minutes, hourHeight) >= BlockLineHeight(minutes);

    /// <summary>A block only earns a second line (its clock) once there is room for two.</summary>
    internal static bool ShowsTime(int minutes, float hourHeight) =>
        BlockLabelSpace(minutes, hourHeight) >= BlockLineHeight(minutes) * 2f;

    /// <summary>The format a block of <paramref name="minutes"/> is written in.</summary>
    internal CanvasTextFormat BlockFormat(int minutes) => minutes < 30 ? BlockSmall : BlockLarge;

    public void Dispose()
    {
        Weekday.Dispose();
        DayNumber.Dispose();
        Hour.Dispose();
        Chrome.Dispose();
        BlockSmall.Dispose();
        BlockLarge.Dispose();
    }

    private static CanvasTextFormat Format(float size, CanvasHorizontalAlignment align) => new()
    {
        FontSize = size,
        HorizontalAlignment = align,
        VerticalAlignment = CanvasVerticalAlignment.Top,
        WordWrapping = CanvasWordWrapping.NoWrap,
        // A title that does not fit ellipsises. It never wraps into a block that has no room for a
        // second line, and it is never simply clipped mid-glyph.
        TrimmingGranularity = CanvasTextTrimmingGranularity.Character,
        TrimmingSign = CanvasTrimmingSign.Ellipsis,
    };
}

/// <summary>
/// The shaped text the grid draws, held, not re-derived.
/// </summary>
/// <remarks>
/// <b>This is the single most expensive thing a calendar frame can do, and the reason a pinch used to
/// cost 3.4× a swipe while drawing half as many blocks.</b> A column's width moves every frame of a
/// pinch, the shaper's cache is keyed on that width, and so every visible label is re-shaped from
/// scratch, sixty times a second, in the gesture the grid is judged on.
/// <para>
/// Two things stop it. The width the text is <i>laid out</i> against is frozen for the length of the
/// gesture (<see cref="CalendarSurfaceState.ShapedDayWidth"/>), the block's rectangle still tracks
/// the fingers every frame, as it must; it is the layout inside it that is held, and it is clipped to
/// the live rectangle anyway. And on top of that, the width is bucketed, so a stray pixel of
/// difference is not a cache miss.
/// </para>
/// </remarks>
internal sealed class TextLayoutCache(ICanvasResourceCreator device) : IDisposable
{
    /// <summary>Widths are floored into buckets this wide. Floored, never rounded up, so a title
    /// still ellipsises inside its own block rather than overflowing it.</summary>
    private const float Bucket = 8f;

    /// <summary>Roughly a busy week's worth of visible labels, twice over.</summary>
    private const int Capacity = 512;

    private readonly Dictionary<(string Text, float Width, int Format), CanvasTextLayout> _cache = [];
    private readonly List<(string Text, float Width, int Format)> _order = [];

    /// <summary>How many layouts were shaped this frame, the number the trace watches.</summary>
    internal int Shaped { get; private set; }

    internal void ResetCounters() => Shaped = 0;

    /// <summary>A shaped, ellipsised line of text, from the cache if it has been seen before.</summary>
    internal CanvasTextLayout Line(string text, CanvasTextFormat format, float maxWidth, int formatId)
    {
        var width = MathF.Max(MathF.Floor(maxWidth / Bucket) * Bucket, Bucket);
        var key = (text, width, formatId);
        if (_cache.TryGetValue(key, out var hit))
        {
            return hit;
        }

        Shaped++;
        var layout = new CanvasTextLayout(device, text, format, width, 0f)
        {
            // The height is left free; the block clips. Constraining it here would let Win2D trim
            // vertically, which drops the line entirely rather than ellipsising it.
            WordWrapping = CanvasWordWrapping.NoWrap,
        };

        if (_cache.Count >= Capacity)
        {
            var oldest = _order[0];
            _order.RemoveAt(0);
            if (_cache.Remove(oldest, out var evicted))
            {
                evicted.Dispose();
            }
        }
        _cache[key] = layout;
        _order.Add(key);
        return layout;
    }

    /// <summary>Throws the lot away, the theme, the locale or the graphics device changed.</summary>
    internal void Clear()
    {
        foreach (var layout in _cache.Values)
        {
            layout.Dispose();
        }
        _cache.Clear();
        _order.Clear();
    }

    public void Dispose() => Clear();
}
