// The renderer's palette, its type, and the one cache that decides whether a pinch holds 60fps.
//
// A canvas has no styles and no resources: a draw call gets colours and text formats or it derives
// them itself, sixty times a second, per block. So they are built once, here, and handed to the
// frame, which is §7's "nothing that a zoom cannot change may be re-derived on a zoom", made
// structural.
using System;
using System.Collections.Generic;
using Microsoft.Graphics.Canvas;
using Microsoft.Graphics.Canvas.Text;
using Microsoft.UI;
using Microsoft.UI.Text;
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
    internal const float BlockGap = CalendarBlocks.Gap;

    /// <summary>The padding before a banner's "+N" chip.</summary>
    internal const float BlockPadding = 2f;

    /// <summary>The type size of an all-day bar's title.</summary>
    internal const float BandFontSize = 11f;

    /// <summary>
    /// The icon font the corner glyphs are drawn in. DirectWrite takes one family, not XAML's
    /// fallback list, so it is chosen by the OS: Segoe Fluent Icons, which Windows 11 (build 22000)
    /// ships, else Windows 10's Segoe MDL2 Assets, which has the same code points.
    /// </summary>
    private static readonly string IconFont =
        OperatingSystem.IsWindowsVersionAtLeast(10, 0, 22000) ? "Segoe Fluent Icons" : "Segoe MDL2 Assets";

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

        // Two rungs, CalendarBlocks.FontSize's: a quarter-hour block at the default horizon is ~11px
        // tall, and a 16px line box slices its title through the middle.
        TitleSmall = Label(CalendarBlocks.FontSize(0), FontWeights.SemiBold);
        TitleLarge = Label(CalendarBlocks.FontSize(60), FontWeights.SemiBold);
        TimeSmall = Label(CalendarBlocks.FontSize(0), FontWeights.Normal);
        TimeLarge = Label(CalendarBlocks.FontSize(60), FontWeights.Normal);
        GlyphSmall = Label(CalendarBlocks.FontSize(0), FontWeights.Normal);
        GlyphSmall.FontFamily = IconFont;
        GlyphLarge = Label(CalendarBlocks.FontSize(60), FontWeights.Normal);
        GlyphLarge.FontFamily = IconFont;
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

    /// <summary>An event's title, semibold, as Apple's Calendar sets it.</summary>
    internal CanvasTextFormat TitleSmall { get; }

    internal CanvasTextFormat TitleLarge { get; }

    /// <summary>The time beneath a block's title.</summary>
    internal CanvasTextFormat TimeSmall { get; }

    internal CanvasTextFormat TimeLarge { get; }

    /// <summary>The corner glyphs beside a <see cref="TitleSmall"/> title.</summary>
    internal CanvasTextFormat GlyphSmall { get; }

    /// <summary>The corner glyphs beside a <see cref="TitleLarge"/> title, and after an all-day bar's.</summary>
    internal CanvasTextFormat GlyphLarge { get; }

    /// <summary>The theme the app is actually showing, read once out of XAML.</summary>
    internal static SurfaceTheme For(ElementTheme theme, bool appIsDark) =>
        new(theme == ElementTheme.Dark || (theme == ElementTheme.Default && appIsDark));

    /// <summary>The title format a block of <paramref name="minutes"/> is written in.</summary>
    internal CanvasTextFormat TitleFormat(int minutes) => minutes < 30 ? TitleSmall : TitleLarge;

    /// <summary>The format of the time beneath it.</summary>
    internal CanvasTextFormat TimeFormat(int minutes) => minutes < 30 ? TimeSmall : TimeLarge;

    /// <summary>The format of the glyphs in its corner.</summary>
    internal CanvasTextFormat GlyphFormat(int minutes) => minutes < 30 ? GlyphSmall : GlyphLarge;

    public void Dispose()
    {
        Weekday.Dispose();
        DayNumber.Dispose();
        Hour.Dispose();
        Chrome.Dispose();
        TitleSmall.Dispose();
        TitleLarge.Dispose();
        TimeSmall.Dispose();
        TimeLarge.Dispose();
        GlyphSmall.Dispose();
        GlyphLarge.Dispose();
    }

    private static CanvasTextFormat Format(float size, CanvasHorizontalAlignment align) => new()
    {
        FontSize = size,
        HorizontalAlignment = align,
        VerticalAlignment = CanvasVerticalAlignment.Top,
        WordWrapping = CanvasWordWrapping.NoWrap,
        TrimmingGranularity = CanvasTextTrimmingGranularity.Character,
        TrimmingSign = CanvasTrimmingSign.Ellipsis,
    };

    /// <summary>An event's label: never trimmed, because a label the block has no room for is cut
    /// at the block's edge or at its last whole line (§4).</summary>
    private static CanvasTextFormat Label(float size, Windows.UI.Text.FontWeight weight) => new()
    {
        FontSize = size,
        FontWeight = weight,
        HorizontalAlignment = CanvasHorizontalAlignment.Left,
        VerticalAlignment = CanvasVerticalAlignment.Top,
        WordWrapping = CanvasWordWrapping.NoWrap,
    };
}

/// <summary>A run of shaped text and what a frame reads of it, read once when it is shaped.</summary>
/// <param name="Width">The widest line.</param>
/// <param name="LineBottoms">Where each line ends, from the top.</param>
internal sealed record ShapedText(CanvasTextLayout Layout, float Width, float Height, float[] LineBottoms);

/// <summary>
/// The shaped text the grid draws, held, not re-derived.
/// </summary>
/// <remarks>
/// Shaping is the most expensive thing a calendar frame can do, and none of it changes while the grid
/// scrolls. A label run is shaped once, unbounded, and cut by the block it is drawn in; a wrapped
/// title is shaped once per width on <see cref="CalendarBlocks.LabelStep"/>'s grid. During a pinch the
/// width a title wraps against is the one frozen for the gesture
/// (<see cref="CalendarSurfaceState.ShapedDayWidth"/>): the block's rectangle still tracks the
/// fingers, and the text inside it is clipped to that.
/// </remarks>
internal sealed class TextLayoutCache(ICanvasResourceCreator device) : IDisposable
{
    /// <summary>The width a run is shaped against: wider than any block, so it never wraps or trims.</summary>
    private const float Unbounded = 100_000f;

    /// <summary>A busy week's labels, each with its time, range and a wrapped title, several times over.</summary>
    private const int Capacity = 2048;

    private readonly Dictionary<(string Text, float Width, CanvasTextFormat Format), ShapedText> _cache = [];
    private readonly Queue<(string Text, float Width, CanvasTextFormat Format)> _order = [];

    /// <summary>How many layouts were shaped this frame, the number the trace watches.</summary>
    internal int Shaped { get; private set; }

    internal void ResetCounters() => Shaped = 0;

    /// <summary>
    /// <paramref name="text"/> on one line, never trimmed. The <paramref name="format"/> is the theme's,
    /// so it names the type in the cache's key; a new theme clears the cache.
    /// </summary>
    internal ShapedText Run(string text, CanvasTextFormat format) =>
        Shape(text, Unbounded, format, CanvasWordWrapping.NoWrap);

    /// <summary>
    /// <paramref name="text"/> wrapped at whole words to <paramref name="width"/>, a multiple of
    /// <see cref="CalendarBlocks.LabelStep"/>. A word wider than that runs past it, which
    /// <see cref="ShapedText.Width"/> shows.
    /// </summary>
    internal ShapedText Wrapped(string text, CanvasTextFormat format, float width) =>
        Shape(text, width, format, CanvasWordWrapping.WholeWord);

    private ShapedText Shape(string text, float width, CanvasTextFormat format, CanvasWordWrapping wrapping)
    {
        var key = (text, width, format);
        if (_cache.TryGetValue(key, out var hit))
        {
            return hit;
        }

        Shaped++;
        // The height is left free; the block clips. Constraining it would let DirectWrite drop the
        // last line rather than leave it to the block to keep only the whole ones.
        var layout = new CanvasTextLayout(device, text, format, width, 0f) { WordWrapping = wrapping };
        var lines = layout.LineMetrics;
        var bottoms = new float[lines.Length];
        var bottom = 0f;
        for (var i = 0; i < lines.Length; i++)
        {
            bottom += lines[i].Height;
            bottoms[i] = bottom;
        }
        var bounds = layout.LayoutBounds;
        var shaped = new ShapedText(layout, (float)bounds.Width, (float)bounds.Height, bottoms);

        if (_cache.Count >= Capacity && _order.TryDequeue(out var oldest) && _cache.Remove(oldest, out var evicted))
        {
            evicted.Layout.Dispose();
        }
        _cache[key] = shaped;
        _order.Enqueue(key);
        return shaped;
    }

    /// <summary>Throws the lot away: the theme, the locale or the graphics device changed.</summary>
    internal void Clear()
    {
        foreach (var shaped in _cache.Values)
        {
            shaped.Layout.Dispose();
        }
        _cache.Clear();
        _order.Clear();
    }

    public void Dispose() => Clear();
}
