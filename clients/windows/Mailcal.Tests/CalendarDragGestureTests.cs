// Which press takes hold of the grid (docs/calendar.md §10): a mouse's drag on a desktop, a finger's
// hold on a touch screen, and never a finger's swipe or two fingers resting before a pinch. The race
// half of Android's `CalendarDragTest`: a hold is a timeout in the same pointer stream as the swipe,
// the pan and the pinch, and the cases here are the ones a hand finds and a script does not.
using System.Collections.Generic;
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarDragGestureTests
{
    private static readonly SurfaceViewport Screen = new(
        Width: 700f, Height: 600f, Gutter: 52f,
        HeaderHeight: 56f, LaneHeight: 24f, DividerHeight: 1f, Lanes: 0);

    private sealed class Recorder(bool accepts = true, bool changes = true) : ICalendarDrag
    {
        internal List<string> Calls { get; } = [];

        public bool Begin(float x, float y)
        {
            Calls.Add($"begin {x},{y}");
            return accepts;
        }

        public void Move(float x, float y) => Calls.Add($"move {x},{y}");

        public bool Finish()
        {
            Calls.Add("finish");
            return changes;
        }

        public void Cancel() => Calls.Add("cancel");
    }

    private static CalendarGestureOwner Owner(Recorder drag, out List<(float X, float Y)> taps)
    {
        var state = new CalendarSurfaceState(12, CalendarUnits.DaysInWeek);
        var driver = new CalendarSurfaceDriver(state);
        var tapped = new List<(float, float)>();
        taps = tapped;
        return new CalendarGestureOwner(
            state, driver, () => Screen,
            touchSlop: 8f,
            onZoomSettled: () => { }, onTap: (x, y) => tapped.Add((x, y)))
        {
            Drag = drag,
        };
    }

    private static PointerSample Mouse(float y, double t) => new(1, 300f, y, t, Mouse: true);

    private static PointerSample Finger(uint id, float x, float y, double t) => new(id, x, y, t);

    [Fact]
    public void A_mouse_drag_on_empty_grid_draws_from_where_it_was_pressed()
    {
        var drag = new Recorder();
        var owner = Owner(drag, out _);

        owner.PointerDown(Mouse(300f, 0));
        owner.PointerMoved(Mouse(305f, 10));
        owner.PointerMoved(Mouse(360f, 20));
        owner.PointerUp(Mouse(360f, 30));

        Assert.Equal(["begin 300,300", "move 300,305", "move 300,360", "finish"], drag.Calls);
    }

    [Fact]
    public void A_mouse_click_is_still_a_click()
    {
        var drag = new Recorder();
        var owner = Owner(drag, out var taps);

        owner.PointerDown(Mouse(300f, 0));
        owner.PointerMoved(Mouse(302f, 10));
        owner.PointerUp(Mouse(302f, 20));

        Assert.Empty(drag.Calls);
        Assert.Single(taps);
    }

    [Fact]
    public void A_mouse_drag_on_nothing_draggable_does_nothing_at_all()
    {
        // On somebody else's meeting, the press is refused: a desktop scrolls with its wheel, so the drag does not
        // pan, and a press that travelled is not a click either.
        var drag = new Recorder(accepts: false);
        var owner = Owner(drag, out var taps);

        owner.PointerDown(Mouse(300f, 0));
        owner.PointerMoved(Mouse(305f, 10));
        owner.PointerMoved(Mouse(400f, 20));
        Assert.Equal(GestureMode.Inert, owner.Mode);
        owner.PointerUp(Mouse(400f, 30));

        Assert.Equal(["begin 300,300"], drag.Calls);
        Assert.Empty(taps);
    }

    [Fact]
    public void A_finger_held_still_takes_hold_where_it_rests()
    {
        var drag = new Recorder();
        var owner = Owner(drag, out _);

        owner.PointerDown(Finger(1, 300f, 300f, 0));
        Assert.True(owner.NeedsTick, "a resting finger needs the clock to become a hold");
        owner.Tick(0.2f);
        owner.PointerMoved(Finger(1, 302f, 301f, 200));
        Assert.Empty(drag.Calls);
        owner.Tick(0.25f);
        Assert.Equal(GestureMode.Drag, owner.Mode);

        owner.PointerMoved(Finger(1, 302f, 420f, 600));
        owner.PointerUp(Finger(1, 302f, 420f, 700));

        Assert.Equal(["begin 302,301", "move 302,420", "finish"], drag.Calls);
    }

    [Fact]
    public void A_swipe_never_becomes_a_hold()
    {
        var drag = new Recorder();
        var owner = Owner(drag, out _);

        owner.PointerDown(Finger(1, 300f, 300f, 0));
        owner.PointerMoved(Finger(1, 300f, 340f, 50));
        Assert.Equal(GestureMode.PanHours, owner.Mode);
        owner.Tick(1f);

        Assert.Equal(GestureMode.PanHours, owner.Mode);
        Assert.Empty(drag.Calls);
    }

    [Fact]
    public void Two_fingers_resting_before_a_slow_pinch_are_not_a_hold()
    {
        // Perfectly still, and so indistinguishable from a hold by time alone.
        var drag = new Recorder();
        var owner = Owner(drag, out _);

        owner.PointerDown(Finger(1, 250f, 300f, 0));
        owner.PointerDown(Finger(2, 400f, 300f, 20));
        owner.Tick(1f);
        owner.PointerUp(Finger(2, 400f, 300f, 1000));
        owner.Tick(1f);

        Assert.Empty(drag.Calls);
    }

    [Fact]
    public void A_cancelled_gesture_draws_nothing()
    {
        var drag = new Recorder();
        var owner = Owner(drag, out _);

        owner.PointerDown(Mouse(300f, 0));
        owner.PointerMoved(Mouse(360f, 10));
        owner.PointerCancelled();

        Assert.Equal(["begin 300,300", "move 300,360", "cancel"], drag.Calls);
    }

    [Fact]
    public void A_second_finger_on_a_drag_neither_pinches_nor_moves_it()
    {
        var drag = new Recorder();
        var owner = Owner(drag, out _);

        owner.PointerDown(Finger(1, 300f, 300f, 0));
        owner.Tick(0.5f);
        owner.PointerDown(Finger(2, 450f, 300f, 600));
        owner.PointerMoved(Finger(2, 520f, 200f, 650));
        owner.PointerMoved(Finger(1, 300f, 360f, 700));

        Assert.Equal(GestureMode.Drag, owner.Mode);
        Assert.Equal(["begin 300,300", "move 300,360"], drag.Calls);
    }

    [Fact]
    public void A_press_that_moved_nothing_on_an_event_is_a_click_on_it()
    {
        // A hand moves a mouse a few pixels while clicking: a move that snaps back to where the event
        // already is writes nothing (§10), and opens the event as the click it was.
        var drag = new Recorder(changes: false);
        var owner = Owner(drag, out var taps);

        owner.PointerDown(Mouse(300f, 0));
        owner.PointerMoved(Mouse(305f, 10));
        owner.PointerUp(Mouse(305f, 20));

        Assert.Equal(["begin 300,300", "move 300,305", "finish"], drag.Calls);
        Assert.Equal([(300f, 300f)], taps);
    }
}
