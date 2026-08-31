// Fast flicks, against a grid that is still moving.
//
// **This is the file that would have caught the swallowed swipe, and no other kind of test could
// have.** The bug needs a gesture to arrive while the *previous gesture's animation is still
// running*. Every test that waits for the grid to settle first, and every synthetic swipe a script
// can inject, touch injection included, politely does the opposite. It reproduced only in a hand,
// moving fast: on Android, eight flicks turned three weeks while every unit test stayed green.
//
// So the clock is taken away from the grid and handed to the test. The gesture owner is fed pointer
// samples, the driver is advanced by an explicit `dt`, and the next flick is delivered ONE FRAME
// after the last, with the week still mid-slide, which is precisely the state that used to eat it.
// That is the whole trick, and it turns a gesture-versus-animation race into a deterministic,
// millisecond-exact test with no window, no UI thread and no test host.
//
// It is the Windows answer to docs/calendar.md §9 and §10: "a test that delivers a gesture while the
// previous animation is still running". UI Automation cannot express this, it has no notion of a
// pointer arriving mid-animation, which is why the grid's gesture logic was deliberately built to
// need no UI at all.
//
// What a released touch gesture does now: it coasts, and comes to rest on the day it stopped nearest,
// the same rule at every zoom, and the same rule the wheel follows (CalendarWheelTests). Nothing is
// banked when the flick is judged, so there is no decision for the next gesture to disagree with, which
// is what the swallowed-swipe tests below are really pinning.
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarFlickTests
{
    /// <summary>One frame at 60Hz, the smallest slice of time a flick can be separated from the last
    /// one by.</summary>
    private const float OneFrameMs = 16f;

    /// <summary>
    /// The grid, driven exactly as the WinUI layer drives it: pointer samples in, render ticks
    /// forward.
    /// </summary>
    /// <remarks>
    /// Nothing here is a mock. It is the same <see cref="CalendarGestureOwner"/> and the same
    /// <see cref="CalendarSurfaceDriver"/> the app runs; only the clock and the pointer source are the
    /// test's.
    /// </remarks>
    private sealed class Harness
    {
        // The whole-week zoom: seven columns across 648px of day viewport, so a week is 648px, and a
        // released swipe lands on one.
        private static readonly SurfaceViewport Screen = new(
            Width: 700f,
            Height: 600f,
            Gutter: 52f,
            HeaderHeight: 56f,
            LaneHeight: 24f,
            DividerHeight: 1f,
            Lanes: 0);

        internal CalendarSurfaceState State { get; }

        internal CalendarSurfaceDriver Driver { get; }

        private readonly CalendarGestureOwner _owner;
        private double _now;
        private uint _nextId = 1;
        private uint _dragId;
        private float _dragX;

        internal Harness(int days = CalendarUnits.DaysInWeek)
        {
            State = new CalendarSurfaceState(visibleHours: 12, visibleDays: days);
            Driver = new CalendarSurfaceDriver(State);
            _owner = new CalendarGestureOwner(
                State,
                Driver,
                () => Screen,
                touchSlop: 10f,
                onZoomSettled: () => { },
                onTap: (_, _) => { });
        }

        internal SurfaceMetrics Metrics => State.Metrics(Screen);

        /// <summary>Where the strip is, in weeks, whole numbers are week boundaries.</summary>
        internal float Position => State.WeekPosition(Metrics);

        /// <summary>Advances the render clock, exactly what CompositionTarget.Rendering does.</summary>
        internal void Advance(float ms)
        {
            _now += ms;
            var dt = ms / 1000f;
            _owner.Tick(dt);
            Driver.Tick(dt, Metrics);
        }

        /// <summary>
        /// A flick: fast, short, and released, the gesture nearly every real page turn is made of.
        /// </summary>
        /// <remarks>
        /// Note what this does <b>not</b> do: tick the driver. A finger is on the glass, so no
        /// animation is running; the clock only moves for the gesture's own samples. Ticking here
        /// would let the previous turn finish sliding, which is the very thing that must not happen.
        /// </remarks>
        private void Flick(float fromX, float toX)
        {
            const int Steps = 5;
            const float StepMs = 10f; // 50ms in total: a flick, not a haul
            var id = _nextId++;
            const float Y = 300f;

            _owner.PointerDown(new PointerSample(id, fromX, Y, _now));
            for (var i = 1; i <= Steps; i++)
            {
                _now += StepMs;
                var x = fromX + ((toX - fromX) * i / Steps);
                _owner.PointerMoved(new PointerSample(id, x, Y, _now));
            }
            _owner.PointerUp(new PointerSample(id, toX, Y, _now));
        }

        /// <summary>Leftwards: brings on the NEXT week.</summary>
        internal void FlickLeft() => Flick(560f, 140f);

        /// <summary>Rightwards: reveals the PREVIOUS week.</summary>
        internal void FlickRight() => Flick(140f, 560f);

        /// <summary>
        /// A slow, deliberate drag, no velocity worth the name, left with the finger still down.
        /// Returns where the strip was when the hand stopped. <see cref="Lift"/> releases it.
        /// </summary>
        /// <remarks>
        /// It <b>holds still</b> before it is lifted, and that is not padding: the velocity tracker fits
        /// over the last 100 ms, so a finger that keeps moving right up to the lift reports a flick
        /// however slowly it was going. Holding fills that window with samples that are not moving,
        /// which is what a hand does when it means "here", and it is the only way to test the drag path
        /// rather than the flick path.
        /// <para>
        /// The strip moves a touch less than the finger, and that is the <b>touch slop</b>, not a bug:
        /// the first few pixels are spent deciding whether this is a sideways drag at all, and are not
        /// applied to anything. Which is exactly why this hands the caller the strip's real position
        /// rather than letting a test assume it equals the finger's travel.
        /// </para>
        /// </remarks>
        internal float DragLeft(float pixels)
        {
            const float From = 600f;
            const float Y = 300f;
            _dragId = _nextId++;
            _owner.PointerDown(new PointerSample(_dragId, From, Y, _now));

            const int Steps = 20;
            for (var i = 1; i <= Steps; i++)
            {
                _now += 30f; // slow: 600ms in total
                _owner.PointerMoved(new PointerSample(_dragId, From - (pixels * i / Steps), Y, _now));
            }
            for (var i = 0; i < 4; i++)
            {
                _now += 30f; // ...and then it stops, for longer than the tracker's window.
                _owner.PointerMoved(new PointerSample(_dragId, From - pixels, Y, _now));
            }

            _dragX = From - pixels;
            return State.StripX;
        }

        /// <summary>Lifts the finger a <see cref="DragLeft"/> left on the glass.</summary>
        internal void Lift() => _owner.PointerUp(new PointerSample(_dragId, _dragX, 300f, _now));

        /// <summary>A drag that is taken away rather than released, capture lost, a system dialog.</summary>
        internal void CancelMidDrag()
        {
            var id = _nextId++;
            _owner.PointerDown(new PointerSample(id, 560f, 300f, _now));
            _now += 10f;
            _owner.PointerMoved(new PointerSample(id, 480f, 300f, _now));
            _owner.PointerCancelled();
        }

        /// <summary>How far the strip is from the nearest day boundary, in columns.</summary>
        internal float OffDay
        {
            get
            {
                var days = Position * CalendarUnits.DaysInWeek;
                return MathF.Abs(days - MathF.Round(days));
            }
        }

        /// <summary>Lets everything in flight run to a stop.</summary>
        internal void Settle()
        {
            for (var i = 0; i < 200 && Driver.IsAnimating; i++)
            {
                Advance(OneFrameMs);
            }
        }
    }

    [Fact]
    public void No_flick_is_swallowed_when_the_next_one_lands_mid_slide()
    {
        // THE regression. Measured on a real phone before the fix: eight fast flicks turned THREE
        // weeks. The swipe was never dropped, the *decision* was. A turn was only committed when its
        // slide finished, so a turn still being drawn had, as far as the state knew, never happened:
        // flick again and the new gesture cancelled the animation of the first, and with it a week
        // already won.
        //
        // **The class of bug is now gone by construction rather than guarded against.** A flick banks
        // no target at all: it adds speed to a strip that coasts, and the landing is whatever day the
        // coast happens to end nearest. There is no decision for a later event to disagree with, which
        // is what let the old code lose one, and is why the driver no longer needs to carry a decision
        // across a cancelled animation.
        var grid = new Harness();
        var last = grid.Position;

        for (var i = 0; i < 8; i++)
        {
            grid.FlickLeft();

            // ONE frame. The grid is still moving when the next finger lands, which is exactly the
            // condition that used to swallow a swipe.
            grid.Advance(OneFrameMs);
            Assert.True(grid.Position > last, $"flick {i + 1} did not move the grid forwards");
            last = grid.Position;
        }
        grid.Settle();

        // Every flick counted, and then some: coasting carries further than a page turn ever did.
        Assert.True(grid.Position >= 8f, $"eight flicks travelled only {grid.Position} weeks");
        Assert.Equal(0f, grid.OffDay, 0.001f);
    }

    [Fact]
    public void A_flick_the_other_way_takes_the_grid_the_other_way_immediately()
    {
        // The same race, reversed halfway. A flick's direction comes from that flick's own finger and
        // takes effect at once, it never has to argue with momentum the previous gesture left behind,
        // and it is never read off the strip's position (which mid-coast carries a lag that looks
        // exactly like a drag the other way).
        //
        // Note it does NOT cancel one-for-one the way page turns did: four on and three back does not
        // land on week one, because a coast carries as far as its speed is worth rather than a fixed
        // page. Reversal is immediate, which is the property a hand can feel; the arithmetic of pages
        // is not.
        var grid = new Harness();

        for (var i = 0; i < 4; i++)
        {
            grid.FlickLeft();
            grid.Advance(OneFrameMs);
        }
        var furthest = grid.Position;
        Assert.True(furthest > 0f);

        var previous = furthest;
        for (var i = 0; i < 3; i++)
        {
            grid.FlickRight();
            grid.Advance(OneFrameMs);
            Assert.True(grid.Position < previous, $"reverse flick {i + 1} did not go backwards");
            previous = grid.Position;
        }
        grid.Settle();

        Assert.True(grid.Position < furthest, "the reversal has to actually reverse");
        Assert.Equal(0f, grid.OffDay, 0.001f);
    }

    [Fact]
    public void However_hard_it_is_flicked_a_touch_swipe_comes_to_rest_on_a_day()
    {
        // The one resting rule, from the roughest input there is: six flicks one way, one back, no time
        // to settle in between. Wherever that leaves the grid, it leaves it on a column boundary.
        var grid = new Harness();

        for (var i = 0; i < 6; i++)
        {
            grid.FlickLeft();
            grid.Advance(OneFrameMs);
        }
        grid.FlickRight();
        grid.Settle();

        Assert.Equal(0f, grid.OffDay, 0.001f);
        Assert.False(grid.Driver.IsAnimating, "the grid never stopped moving");
    }

    [Fact]
    public void A_short_touch_drag_finishes_the_column_rather_than_undoing_it()
    {
        // The half-swipe: a slow drag, released with no speed in it. It used to be measured against a
        // page-turn threshold and, falling short, sent all the way home, the user's 60px of travel
        // simply deleted.
        //
        // Now there is no threshold to fall short of. 60px is two thirds of a column, so it rests on
        // the near side of the next one: the grid finishes what the hand started. Nothing the user did
        // is ever discarded, which is the property a threshold cannot have.
        var grid = new Harness();
        var held = grid.DragLeft(60f);
        Assert.True(held > 0f, "the drag moved the strip");

        grid.Lift();
        grid.Settle();

        Assert.Equal(0, grid.State.Week);
        Assert.Equal(grid.Metrics.DayWidth, grid.State.StripX, 0.5f);
    }

    [Fact]
    public void A_touch_drag_at_a_sub_week_zoom_rests_on_a_day_too()
    {
        // Three columns, same rule. It is still never pulled onto a WEEK boundary, that would forbid
        // the user Wednesday-to-Friday, which is the whole reason to have asked for three columns, but
        // a column edge is a column edge at every zoom, so there is no longer a "does this zoom snap?"
        // question anywhere in the state machine.
        var grid = new Harness(days: 3);
        var held = grid.DragLeft(150f);
        Assert.True(held > 0f);

        grid.Lift();
        grid.Settle();

        Assert.Equal(grid.Metrics.DayWidth, grid.State.StripX, 0.5f); // 216px: one column of three
        Assert.Equal(0, grid.State.Week);
        Assert.Equal(0f, grid.OffDay, 0.001f);
    }

    [Fact]
    public void The_strip_never_leaves_its_anchor_week_however_fast_it_is_flicked()
    {
        // What replaced the old page lag. The pixels used to be allowed to trail the week they had
        // already landed on by up to two whole pages, so the grid was sliding THROUGH weeks and had to
        // hold five of them or draw a hole. The strip re-anchors instead: the offset is kept inside its
        // own week, so at most one seam, at most two weeks, is ever on screen. Flick far faster than
        // it can catch up, and it still holds.
        var grid = new Harness();
        var week = grid.Metrics.WeekWidth;

        for (var i = 0; i < 20; i++)
        {
            grid.FlickLeft();

            // Barely a millisecond: nothing has time to catch up.
            grid.Advance(1f);
            Assert.InRange(grid.State.StripX, 0f, week);
        }
        grid.Settle();
        Assert.InRange(grid.State.StripX, 0f, week);

        // Twenty flicks, all of them counted, and resting on a column.
        Assert.True(grid.Position >= 16f, $"twenty flicks travelled only {grid.Position} weeks");
        Assert.Equal(0f, grid.OffDay, 0.001f);
    }

    [Fact]
    public void Jumping_home_leaves_nothing_behind_for_the_next_swipe_to_build_on()
    {
        // "Back to today" teleports the strip to the origin. A swipe straight afterwards must travel
        // from HOME, not land back near the week the user just asked to leave.
        //
        // This used to need care: a flick banked its target week up front, and that banked value had to
        // be explicitly voided here or the next swipe added to it. Nothing is banked any more, so there
        // is nothing to void, the swipe reads the strip, and the strip is at home.
        var grid = new Harness();
        for (var i = 0; i < 3; i++)
        {
            grid.FlickLeft();
            grid.Advance(OneFrameMs);
        }
        grid.Settle();
        var travelled = grid.Position;
        Assert.True(travelled >= 3f);

        // Exactly what CalendarSurface.Recentre does.
        grid.Driver.Stop();
        grid.State.ResetWeek();
        Assert.Equal(0, grid.State.Week);

        grid.FlickLeft();
        grid.Settle();

        Assert.True(grid.Position > 0f, "the swipe moved");
        Assert.True(
            grid.Position < travelled,
            $"a swipe from home landed at {grid.Position}, it built on the weeks left behind");
        Assert.Equal(0f, grid.OffDay, 0.001f);
    }

    [Fact]
    public void A_gesture_taken_away_mid_swipe_still_lands_on_a_day()
    {
        // A system dialog, the window deactivating, pointer capture torn off. None of the release path
        // runs, so without this the grid sits resting between two columns for as long as the user
        // looks at it.
        var grid = new Harness();
        grid.FlickLeft();
        grid.Advance(OneFrameMs);

        // A second gesture starts, drags a little, and is then cancelled rather than lifted.
        grid.CancelMidDrag();
        grid.Settle();

        Assert.Equal(0f, grid.OffDay, 0.001f);
        Assert.False(grid.Driver.IsAnimating);
    }
}
