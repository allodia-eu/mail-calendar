// The tap hit-test, pinned headlessly: a point inside an event's drawn rectangle hits it, a point
// beside it misses, and a band collapsed under the "+N" chip is never hittable. This is the pointer
// side of §7, the same SurfaceMetrics.BlockRect / BandRect the canvas draws with and the automation
// overlay speaks from, so if the arithmetic ever drifts from the renderer's, this fails rather than a
// finger silently landing on the wrong event (which no screen would show).
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarHitTestTests
{
    // A grid with round numbers: 100px day columns, a 30px hour, a 50px hour-ruler gutter, a 40px
    // header and a two-lane, 20px-per-lane banner (so ContentTop = 40 + 40 + 1 = 81).
    private static SurfaceMetrics Metrics()
    {
        var viewport = new SurfaceViewport(
            Width: 770f, Height: 600f, Gutter: 50f, HeaderHeight: 40f,
            LaneHeight: 20f, DividerHeight: 1f, Lanes: 2);
        return new SurfaceMetrics(viewport, HourHeight: 30f, DayWidth: 100f, BannerLanes: 2);
    }

    [Fact]
    public void A_timed_block_is_hit_inside_its_translated_rect_and_missed_beside_it()
    {
        var m = Metrics();
        // Day 1, 09:00–10:00, full column. BlockRect = (100, 270, 200, 300) in week coords; translated
        // by dx=50 and dy=ContentTop(81) → (150, 351, 250, 381).
        var block = new BlockSpan(Day: 1, Column: 0, Columns: 1, StartMinutes: 540, EndMinutes: 600);
        var blocks = new[] { block };

        Assert.Equal(0, CalendarHitTest.BlockAt(blocks, m, dx: 50f, scrollY: 0f, x: 200f, y: 360f));
        Assert.Equal(-1, CalendarHitTest.BlockAt(blocks, m, dx: 50f, scrollY: 0f, x: 140f, y: 360f)); // left of it
        Assert.Equal(-1, CalendarHitTest.BlockAt(blocks, m, dx: 50f, scrollY: 0f, x: 200f, y: 340f)); // above it
    }

    [Fact]
    public void A_block_scrolled_up_moves_its_hit_rect_with_the_scroll()
    {
        var m = Metrics();
        var blocks = new[] { new BlockSpan(1, 0, 1, 540, 600) };
        // Scrolled down 100px: the rect moves up, so the point that was inside is now above it, and a
        // point 100px higher is inside.
        Assert.Equal(-1, CalendarHitTest.BlockAt(blocks, m, dx: 50f, scrollY: 100f, x: 200f, y: 360f));
        Assert.Equal(0, CalendarHitTest.BlockAt(blocks, m, dx: 50f, scrollY: 100f, x: 200f, y: 260f));
    }

    [Fact]
    public void An_all_day_band_is_hit_in_the_banner()
    {
        var m = Metrics();
        // Day 2, one column, lane 0. BandRect = (200, 0, 300, 20); translated by dx=50, dy=HeaderHeight
        // (40) → (250, 40, 350, 60).
        var bands = new[] { new BandSpan(Day: 2, Days: 1, Lane: 0) };
        Assert.Equal(0, CalendarHitTest.BandAt(bands, drawnLanes: 2, m, dx: 50f, x: 300f, y: 50f));
        Assert.Equal(-1, CalendarHitTest.BandAt(bands, drawnLanes: 2, m, dx: 50f, x: 300f, y: 70f)); // below the lane
    }

    [Fact]
    public void A_band_in_a_collapsed_lane_is_never_hittable()
    {
        var m = Metrics();
        // A bar in lane 1, but only one lane is drawn, it is hidden under the "+N" chip, so a tap
        // where it would be must fall through (return -1), never open a hidden event.
        var bands = new[] { new BandSpan(Day: 0, Days: 1, Lane: 1) };
        // Its rect (translated) would be (50, 60, 150, 80); the point is inside that, but lane 1 ≥ drawn.
        Assert.Equal(-1, CalendarHitTest.BandAt(bands, drawnLanes: 1, m, dx: 50f, x: 100f, y: 70f));
        Assert.Equal(0, CalendarHitTest.BandAt(bands, drawnLanes: 2, m, dx: 50f, x: 100f, y: 70f)); // drawn: hittable
    }
}
