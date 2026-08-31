// A tap has to hit exactly what the frame drew.
//
// This is §7 of the calendar contract on the pointer side: a finger, a screen reader and the pixels
// must all agree, so the hit-test is computed by the SAME SurfaceMetrics.BlockRect / BandRect the
// canvas draws with and the automation overlay speaks from (CalendarSurfaceAutomation / .Spoken),
// translated by the SAME strip and scroll offsets. A second copy of that arithmetic would be a second
// chance to disagree, and a tap landing on the wrong event is a bug no sighted test would ever see.
//
// It is pure, BlockSpan / BandSpan / SurfaceMetrics carry no WinUI type, so it links straight into
// Mailcal.Tests and the whole "does this point fall on this event" contract is pinned headlessly,
// exactly like the flick race. The surface (CalendarSurface.Input) computes the per-week strip offset
// and maps the returned index back to the block's (account, event); this file owns only the geometry.
using System.Collections.Generic;

namespace Allodia.Mailcal.Calendar;

/// <summary>The pointer hit-test for the drawn time grid, the renderer's own geometry, inverted.</summary>
internal static class CalendarHitTest
{
    /// <summary>
    /// The index of the first all-day band whose translated rect contains
    /// (<paramref name="x"/>, <paramref name="y"/>), or <c>-1</c>.
    /// </summary>
    /// <remarks>
    /// <paramref name="dx"/> is the week's strip offset (<c>gutter + LeftOf(week)</c>), matching the
    /// banner translation in <c>CalendarSurface.Spoken</c>. Only bands in a <b>drawn</b> lane are
    /// hittable, a bar collapsed under the "+N" chip is not on screen, so a tap there must fall
    /// through to the chip, never open a hidden event.
    /// </remarks>
    internal static int BandAt(
        IReadOnlyList<BandSpan> bands, int drawnLanes, SurfaceMetrics m, float dx, float x, float y)
    {
        var dy = m.Viewport.HeaderHeight;
        for (var i = 0; i < bands.Count; i++)
        {
            var span = bands[i];
            if (span.Lane < drawnLanes && m.BandRect(span).Translate(dx, dy).Contains(x, y))
            {
                return i;
            }
        }
        return -1;
    }

    /// <summary>
    /// The index of the first timed block whose translated rect contains
    /// (<paramref name="x"/>, <paramref name="y"/>), or <c>-1</c>.
    /// </summary>
    /// <remarks>
    /// The grid translation the canvas draws with: the same <paramref name="dx"/> horizontally, and
    /// <c>ContentTop - scrollY</c> vertically. Blocks are checked in list order; the core does not
    /// overlap two blocks in one column, so the first hit is the only hit.
    /// </remarks>
    internal static int BlockAt(
        IReadOnlyList<BlockSpan> blocks, SurfaceMetrics m, float dx, float scrollY, float x, float y)
    {
        var dy = m.ContentTop - scrollY;
        for (var i = 0; i < blocks.Count; i++)
        {
            if (m.BlockRect(blocks[i]).Translate(dx, dy).Contains(x, y))
            {
                return i;
            }
        }
        return -1;
    }
}
