// The accessibility overlay's DIP→physical conversion, pinned headlessly. The grid's geometry is in
// DIPs; UIA reports bounding rectangles in physical pixels, so CalendarItemPeer must scale a node's
// DIP rect by the display's rasterization factor before adding it to the (physical) surface origin.
// The first Windows-verified build of the calendar found this the hard way: on a 200% display every
// spoken/touch-exploration rect drifted toward the surface's top-left, worse the further from it, so a
// screen reader (and any UIA client) located events far from where they were drawn, a bug invisible on
// a 100% display, where scale is 1 and the conversion is the identity. These tests fail if that scaling
// is ever dropped again (docs/calendar.md §4).
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class GridRectScreenTests
{
    [Fact]
    public void At_100_percent_the_conversion_is_origin_plus_offset()
    {
        // scale 1.0: the DIP offset is already the pixel offset, so this is the plain addition the old
        // code did, the case that (coincidentally) worked and hid the bug on unscaled displays.
        var r = new GridRect(Left: 56f, Top: 115f, Right: 56f + 151f, Bottom: 115f + 57f);
        var (x, y, w, h) = r.ToScreen(originX: 544d, originY: 285d, scale: 1d);
        Assert.Equal(600d, x);
        Assert.Equal(400d, y);
        Assert.Equal(151d, w);
        Assert.Equal(57d, h);
    }

    [Fact]
    public void At_200_percent_offsets_and_size_scale_so_the_node_lands_where_it_is_drawn()
    {
        // The reproduction: an event whose DIP top-left is (56, 115) from the surface origin. Drawn, it
        // is (56, 115) × 2 = (112, 230) pixels down/right of the physical origin (544, 285) → (656, 515),
        // sized 302 × 114. The pre-fix code returned (600, 400), 115px too high, one event-row off.
        var r = new GridRect(Left: 56f, Top: 115f, Right: 56f + 151f, Bottom: 115f + 57f);
        var (x, y, w, h) = r.ToScreen(originX: 544d, originY: 285d, scale: 2d);
        Assert.Equal(656d, x);
        Assert.Equal(515d, y);
        Assert.Equal(302d, w);
        Assert.Equal(114d, h);
    }

    [Fact]
    public void The_error_grows_with_distance_from_the_origin()
    {
        // A node far to the right (a later day column) drifts more than a near one, the tell-tale of an
        // unscaled offset. Near the origin the two agree closely; far from it they must not.
        var near = new GridRect(10f, 10f, 20f, 20f);
        var far = new GridRect(800f, 500f, 810f, 510f);

        var (nx, _, _, _) = near.ToScreen(0d, 0d, 2d);
        var (fx, fy, _, _) = far.ToScreen(0d, 0d, 2d);

        Assert.Equal(20d, nx);        // near: 10 × 2
        Assert.Equal(1600d, fx);      // far: 800 × 2, a 800px pixel error had it not been scaled
        Assert.Equal(1000d, fy);      // far: 500 × 2
    }
}
