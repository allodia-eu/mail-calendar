// The folder pane's width bounds (docs/folder-pane.md).
//
// The drag that moves the boundary cannot be tested here, or by the UI suite: synthetic pointer
// input does not reach this client's WinUI content at all, the list|reading splitter that has
// shipped for months is exactly as undrivable, which is how we know it is the input path and not
// the handler. What CAN be pinned is the arithmetic, and that is where the sharp edge is: on a
// narrow window the pane's floor and the content's floor cross, and Math.Clamp with min > max
// throws rather than returning anything.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SidebarWidthTests
{
    // A comfortably wide window: the window's own limit is far away, so only the pane's bounds bite.
    private const double Roomy = 2000;

    [Fact]
    public void A_width_inside_the_bounds_is_left_alone()
    {
        Assert.Equal(360, SidebarWidth.Clamp(360, Roomy));
        Assert.Equal(SidebarWidth.Default, SidebarWidth.Clamp(SidebarWidth.Default, Roomy));
    }

    [Fact]
    public void Dragging_past_either_end_stops_at_it()
    {
        Assert.Equal(SidebarWidth.Min, SidebarWidth.Clamp(40, Roomy));
        Assert.Equal(SidebarWidth.Max, SidebarWidth.Clamp(5000, Roomy));
    }

    [Fact]
    public void The_window_narrows_the_ceiling_before_the_pane_crowds_the_mail()
    {
        // 900 wide, 480 of which the mail keeps: the pane may reach 420, not its own 560 maximum.
        Assert.Equal(420, SidebarWidth.Clamp(560, 900));
        // …and a width already inside that narrower ceiling is untouched.
        Assert.Equal(300, SidebarWidth.Clamp(300, 900));
    }

    [Fact]
    public void A_window_too_narrow_for_both_floors_keeps_the_pane_rather_than_throwing()
    {
        // The crossed-bounds case. Math.Clamp(x, 200, 120) throws, so this is not a hypothetical
        // tidy-up: a window dragged narrow enough would take the app down with it.
        foreach (var available in new double[] { 600, 500, 300, 0 })
        {
            Assert.Equal(SidebarWidth.Min, SidebarWidth.Clamp(400, available));
        }
    }

    [Fact]
    public void The_bounds_leave_room_for_a_real_two_pane_window()
    {
        // A sanity check on the constants rather than the function: the minimum window that can
        // honour both floors has to be something a person would actually use.
        Assert.True(SidebarWidth.Min + SidebarWidth.MinContent <= 700,
            "the pane and the mail must both fit on a small laptop window");
        Assert.True(SidebarWidth.Min < SidebarWidth.Default && SidebarWidth.Default < SidebarWidth.Max,
            "the default has to sit between the ends the user can drag to");
    }
}
