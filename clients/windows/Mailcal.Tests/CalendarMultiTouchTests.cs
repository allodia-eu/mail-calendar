// Two fingers down, two fingers up, and the grid must know it is empty-handed at the end.
//
// The regression this pins: on Windows the shell captured each touch contact and released it, but
// gated the whole gesture on a single bool. A pinch is TWO contacts, so the first finger's release
// flipped the bool and the second finger's release was ignored, its capture stranded. Windows then
// believed touch was still owned by a lifted contact and routed every new touch to nowhere: the
// touchscreen went dead (the touchpad, which sends wheel rather than captured pointers, kept
// working), and the leaks piled up until the app fell over.
//
// The shell's capture set is not unit-testable without a window, but the invariant it MUST mirror
// is: the pure owner stops tracking only when the LAST of several contacts lifts, and reports that
// honestly through IsTracking. If that ever regressed, the shell built on top of it would leak no
// matter how carefully it counted. So this is the load-bearing half, and it is testable.
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarMultiTouchTests
{
    private static SurfaceViewport Screen => new(
        Width: 700f, Height: 600f, Gutter: 52f,
        HeaderHeight: 56f, LaneHeight: 24f, DividerHeight: 1f, Lanes: 0);

    private static CalendarGestureOwner Owner(out CalendarSurfaceState state)
    {
        state = new CalendarSurfaceState(12, CalendarUnits.DaysInWeek);
        var driver = new CalendarSurfaceDriver(state);
        return new CalendarGestureOwner(
            state, driver, () => Screen,
            touchSlop: 8f,
            onZoomSettled: () => { }, onTap: (_, _) => { });
    }

    [Fact]
    public void Two_fingers_up_leaves_the_owner_empty_handed()
    {
        // The exact shape of a pinch's lifecycle. If the owner still thought a contact was down after
        // both had lifted, the shell mirroring it would hold a capture forever, the leak.
        var owner = Owner(out _);

        owner.PointerDown(new PointerSample(1, 300f, 300f, 0));
        Assert.True(owner.IsTracking);

        owner.PointerDown(new PointerSample(2, 400f, 300f, 5));
        Assert.True(owner.IsTracking);

        // First finger up, the owner is STILL tracking, because one contact remains. This is the
        // frame the old single-bool shell got wrong: it declared the gesture over here.
        owner.PointerUp(new PointerSample(1, 300f, 300f, 40));
        Assert.True(owner.IsTracking, "one finger is still down, the gesture is not over");

        // Second finger up, now, and only now, is it empty-handed.
        owner.PointerUp(new PointerSample(2, 400f, 300f, 45));
        Assert.False(owner.IsTracking, "both fingers lifted, nothing should still be held");
    }

    [Fact]
    public void A_pinch_then_a_swipe_both_track_and_both_clear()
    {
        // Do it twice, because the leak was cumulative: the app degraded over a minute of pinching,
        // not on the first one. A second full gesture must start and end just as clean as the first.
        var owner = Owner(out _);

        // A pinch.
        owner.PointerDown(new PointerSample(1, 300f, 300f, 0));
        owner.PointerDown(new PointerSample(2, 400f, 300f, 5));
        owner.PointerMoved(new PointerSample(1, 260f, 300f, 20));
        owner.PointerMoved(new PointerSample(2, 460f, 300f, 20));
        owner.PointerUp(new PointerSample(1, 260f, 300f, 40));
        owner.PointerUp(new PointerSample(2, 460f, 300f, 45));
        Assert.False(owner.IsTracking, "the pinch left a contact behind");

        // A one-finger swipe, immediately after.
        owner.PointerDown(new PointerSample(3, 500f, 300f, 100));
        Assert.True(owner.IsTracking);
        owner.PointerMoved(new PointerSample(3, 200f, 300f, 120));
        owner.PointerUp(new PointerSample(3, 200f, 300f, 140));
        Assert.False(owner.IsTracking, "the swipe left a contact behind");
    }

    [Fact]
    public void A_cancel_clears_every_contact_at_once()
    {
        // A system dialog, or the window deactivating, mid-pinch. Both contacts are gone at once, and
        // the shell's recovery path leans on the owner agreeing that nothing is held.
        var owner = Owner(out _);
        owner.PointerDown(new PointerSample(1, 300f, 300f, 0));
        owner.PointerDown(new PointerSample(2, 400f, 300f, 5));
        Assert.True(owner.IsTracking);

        owner.PointerCancelled();
        Assert.False(owner.IsTracking, "a cancel must drop every contact");
    }
}
