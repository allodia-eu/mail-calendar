// How an unanswered invitation looks on the calendar: a dashed border, a hatched leading gutter, and
// a fill that reads as provisional rather than booked.
//
// The core decides *which* records are holds (`participation == NeedsAction`, docs/invitations.md);
// this file is only the drawing, kept in one place so the grid block, the all-day bar, the month chip
// and the invitation card's preview cannot drift apart. The deliberate twin of Android's
// `CalendarParticipation.kt`, constant for constant.
//
// Every piece here is a no-op on an answered record, so nothing about a confirmed commitment's
// appearance changes: a hold is told apart by shape, not by a restyle of everything around it.
//
// **Two renderers, one set of numbers.** The week grid and the all-day banner are a Win2D canvas; the
// month chips and the card's preview are composed WinUI. Android delegates its composed path back to
// the canvas one, which is not available here, a `CanvasControl` per month chip would cost far more
// than the hatch is worth. So the hatch exists twice, immediately below each other, over one set of
// constants. If you change the step or the dash, you are changing both by construction.
//
// The visual is never the whole disclosure. A dashed border is invisible to a screen reader, so every
// surface that draws a hold also says it, `InvitationFormat.SpokenWithHold` appends "Awaiting your
// response" (docs/calendar.md §4, the spoken-grid rule).
using System;
using Microsoft.Graphics.Canvas;
using Microsoft.Graphics.Canvas.Geometry;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;
using Windows.UI;

namespace Allodia.Mailcal.Calendar;

/// <summary>The hold treatment, for both of the client's calendar renderers.</summary>
internal static class CalendarHold
{
    /// <summary>The width of the hatched gutter down a hold's leading edge, and the diagonals' pitch.</summary>
    private const float Gutter = 4f;
    private const float HatchStep = 4f;

    /// <summary>The hairline every surface strokes with, solid on a commitment, dashed on a hold.</summary>
    private const float EdgeThickness = 1f;

    /// <summary>The dash a hold's border is stroked with: on, then off, in multiples of the width.</summary>
    private static readonly float[] Dash = [3f, 2f];

    /// <summary>Cached, because a stroke style is a device-independent resource and a frame must not
    /// allocate one per block (§7).</summary>
    private static readonly CanvasStrokeStyle Dashed = new() { CustomDashStyle = Dash };

    private static readonly CanvasStrokeStyle Solid = new();

    /// <summary>A colour faded to <see cref="InvitationFormat.HoldFillAlpha"/> on a hold, untouched
    /// on a commitment. Applied when the page is built, never inside a frame.</summary>
    internal static Color Fade(Color color, bool awaiting) =>
        Color.FromArgb(InvitationFormat.HoldAlpha(color.A, awaiting), color.R, color.G, color.B);

    /// <summary>
    /// The style a record's edge is stroked with: dashed for an unanswered hold, the grid's own
    /// hairline otherwise.
    /// </summary>
    /// <remarks>
    /// A record that already draws a border strokes it with this rather than gaining a second, which
    /// is why the grid block calls this and <see cref="Hatch"/>, while a surface with no border of its
    /// own calls <see cref="Draw"/> instead.
    /// </remarks>
    internal static CanvasStrokeStyle Stroke(bool awaiting) => awaiting ? Dashed : Solid;

    /// <summary>
    /// The diagonal hatching down a hold's leading edge, the part of the treatment that survives
    /// being looked at quickly, when a dashed border at a zoomed-out hour height does not.
    /// </summary>
    /// <remarks>Draws nothing unless <paramref name="awaiting"/>, so a commitment costs one test.</remarks>
    internal static void Hatch(CanvasDrawingSession ds, Rect rect, Color color, bool awaiting)
    {
        if (!awaiting)
        {
            return;
        }
        var width = MathF.Min(Gutter, (float)rect.Width);
        var height = (float)rect.Height;
        if (width <= 0f || height <= 0f)
        {
            return;
        }
        var left = (float)rect.X;
        var top = (float)rect.Y;
        using var layer = ds.CreateLayer(1f, new Rect(left, top, width, height));
        // Start a full height to the left, so the first stripe already crosses the strip rather than
        // leaving its top corner bare.
        for (var x = left - height; x < left + width + height; x += HatchStep)
        {
            ds.DrawLine(x, top + height, x + height, top, color, EdgeThickness);
        }
    }

    /// <summary>
    /// The whole treatment for a canvas surface that has <b>no</b> border of its own, the hatched
    /// gutter and the dashed edge in one call. A no-op on a commitment.
    /// </summary>
    internal static void Draw(CanvasDrawingSession ds, Rect rect, Color color, float corner, bool awaiting)
    {
        if (!awaiting)
        {
            return;
        }
        Hatch(ds, rect, color, awaiting: true);
        // Inset by half the stroke, which straddles the path it is given: drawn on the boundary its
        // outer half falls outside the chip and is clipped away, leaving a half-weight dash that reads
        // as a rendering artefact rather than as a deliberate edge.
        const float half = EdgeThickness / 2f;
        if (rect.Width <= half * 2 || rect.Height <= half * 2)
        {
            return;
        }
        var inset = new Rect(
            rect.X + half, rect.Y + half, rect.Width - (half * 2), rect.Height - (half * 2));
        ds.DrawRoundedRectangle(inset, corner, corner, color, EdgeThickness, Dashed);
    }

    /// <summary>
    /// The same treatment for a <b>composed</b> surface, the month grid's chips and the invitation
    /// card's preview, which are WinUI elements rather than draw calls.
    /// </summary>
    /// <remarks>
    /// Returns <paramref name="content"/> unchanged on a commitment, so a month cell of ordinary
    /// appointments is element-for-element what it was and pays nothing.
    /// <para>
    /// The hatch is a handful of <see cref="Line"/>s in a clipped <see cref="Canvas"/> rather than a
    /// repeating brush: a hold chip is ~16 px tall, so it is four or five lines, and a shape whose
    /// geometry is stated outright cannot be surprised by a brush's tiling rules on a fractional
    /// scale factor.
    /// </para>
    /// </remarks>
    internal static FrameworkElement Compose(
        FrameworkElement content, Color color, double corner, bool awaiting, double height)
    {
        if (!awaiting)
        {
            return content;
        }
        var brush = new SolidColorBrush(color);
        var host = new Grid();
        host.Children.Add(content);
        host.Children.Add(new Rectangle
        {
            RadiusX = corner,
            RadiusY = corner,
            Stroke = brush,
            StrokeThickness = EdgeThickness,
            StrokeDashArray = [.. Dash],
            // A stroke is not a hit target here, the chip beneath keeps its own tap.
            IsHitTestVisible = false,
        });
        if (height > 0)
        {
            host.Children.Add(HatchStrip(brush, height));
        }
        return host;
    }

    /// <summary>The composed hatch: diagonals across the leading gutter, clipped to it.</summary>
    private static UIElement HatchStrip(Brush brush, double height)
    {
        var strip = new Canvas
        {
            Width = Gutter,
            Height = height,
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Stretch,
            IsHitTestVisible = false,
            Clip = new RectangleGeometry { Rect = new Rect(0, 0, Gutter, height) },
        };
        for (var x = -height; x < Gutter + height; x += HatchStep)
        {
            strip.Children.Add(new Line
            {
                X1 = x,
                Y1 = height,
                X2 = x + height,
                Y2 = 0,
                Stroke = brush,
                StrokeThickness = EdgeThickness,
            });
        }
        return strip;
    }
}
