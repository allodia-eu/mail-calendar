// The wheel, a mouse, and a precision touchpad's digested two-finger pan.
//
// A touchpad never hands the app raw contacts (§6): Windows digests a two-finger pan into wheel
// messages. What this file pins, all of it found on a real trackpad:
//
//   1. **A pan stays where it stopped.** This is the regression the whole strip rewrite is for. The
//      grid used to infer the end of a wheel gesture from a brief silence and then resolve the
//      accumulated drag onto a week, commit a turn, or spring home. A slow pan is *mostly* silence,
//      so it sprang home, over and over: scrolling gently on a trackpad, the calendar rubber-banded
//      back to the week under the user's fingers six times in thirteen seconds. There is no threshold
//      that fixes that, because the gesture tripping it is *a pan that has not finished yet*. The fix
//      is to stop resolving pans at all, which the continuous strip and the pinned ruler make safe.
//   2. The horizontal wheel's sign is inverted vs the vertical one (a WM_MOUSEHWHEEL quirk).
//   3. A Ctrl+wheel *zoom* still has no lift, and still has to persist its settled shape to the core
//      exactly once, so silence remains the only signal available for that, and the idle window is
//      still kept short.
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarWheelTests
{
    // The whole-week zoom: seven columns across 648px of day viewport, so a week is 648px wide.
    private static SurfaceViewport Screen => new(
        Width: 700f, Height: 600f, Gutter: 52f,
        HeaderHeight: 56f, LaneHeight: 24f, DividerHeight: 1f, Lanes: 0);

    /// <summary>A week, in pixels, at this fixture's zoom, the distance a pan has to cover to cross one.</summary>
    private const float WeekPx = 648f;

    private sealed class Rig
    {
        internal CalendarSurfaceState State { get; } = new(visibleHours: 12, visibleDays: CalendarUnits.DaysInWeek);
        internal CalendarSurfaceDriver Driver { get; }
        internal CalendarGestureOwner Owner { get; }

        /// <summary>How many times a settled zoom has been handed back to the host to persist.</summary>
        internal int ZoomsSettled { get; private set; }

        internal Rig()
        {
            Driver = new CalendarSurfaceDriver(State);
            Owner = new CalendarGestureOwner(
                State, Driver, () => Screen, touchSlop: 8f,
                onZoomSettled: () => ZoomsSettled++, onTap: (_, _) => { });
        }

        internal void Pan(float delta, int notches)
        {
            for (var i = 0; i < notches; i++)
            {
                Owner.Wheel(delta, horizontal: true, control: false, x: 350f, y: 300f);
            }
        }

        /// <summary>One render frame, exactly as CalendarSurface.OnRendering runs it: owner first (its
        /// idle timer may close a gesture), then the driver advances whatever is moving.</summary>
        internal void Frame(float dtMs)
        {
            var dt = dtMs / 1000f;
            var before = State.WeekPosition(State.Metrics(Screen));
            Owner.Tick(dt);
            Driver.Tick(dt, State.Metrics(Screen));
            Frames++;
            if (MathF.Abs(State.WeekPosition(State.Metrics(Screen)) - before) > 1e-6f)
            {
                FramesThatMoved++;
            }
        }

        /// <summary>Frames run, and how many of them actually moved the grid.</summary>
        internal int Frames { get; private set; }

        internal int FramesThatMoved { get; private set; }

        /// <summary>
        /// Delivers notches at a real input cadence, a notch, then the frames that elapse before the
        /// next one. A mouse's measured gap is ~150 ms; a trackpad streams an order of magnitude denser.
        /// </summary>
        internal void PanAtCadence(float delta, int notches, float gapMs)
        {
            for (var i = 0; i < notches; i++)
            {
                Owner.Wheel(delta, horizontal: true, control: false, x: 350f, y: 300f);
                for (var f = 0; f < (int)(gapMs / 16f); f++)
                {
                    Frame(16f);
                }
            }
        }

        /// <summary>Delivers notches and then lets the wheel fall quiet, which is how a scroll ends.</summary>
        internal void PanAndRest(float delta, int notches)
        {
            Pan(delta, notches);
            RunToRest();
        }

        /// <summary>Runs frames until nothing is moving, or the cap is hit.</summary>
        internal void RunToRest()
        {
            for (var i = 0; i < 240 && (Driver.IsAnimating || Owner.NeedsTick); i++)
            {
                Frame(16f);
            }
        }
    }

    [Fact]
    public void A_slow_wheel_pan_never_springs_back_to_the_week_it_came_from()
    {
        // **THE regression.** Three gentle notches, nowhere near the old page-turn threshold (a sixth
        // of 648px is 108px), and then the wheel falls quiet, which is all a trackpad ever does. The
        // old grid took that silence for a lift, judged 60px of travel as "he didn't mean it", and
        // sprang the week back under the user's hand.
        //
        // It lands on the nearest DAY now, which is forwards: 60px is two thirds of a column, so the
        // grid finishes the column the user started rather than undoing it. That is the whole
        // difference. A landing at most half a column away can only read as settling; a week-sized one
        // read as being overruled.
        var r = new Rig();
        r.Pan(delta: 20f, notches: 3);
        r.RunToRest();

        var dayWidth = r.State.Metrics(Screen).DayWidth; // 92.57px
        Assert.Equal(dayWidth, r.State.StripX, 0.5f);
        Assert.Equal(0, r.State.Week);
        Assert.True(r.State.StripX > 0f, "it must never end up back where the gesture began");
        Assert.False(r.Driver.IsAnimating, "and then it is finished");
    }

    [Fact]
    public void A_notch_keeps_the_grid_moving_until_the_next_one_arrives()
    {
        // **The staircase.** A wheel has no phase, so applying each notch the instant it lands moves the
        // grid once and leaves it perfectly still until the next one. Measured on a real mouse at
        // ~150 ms between notches that drew 16-31 fps of a 60 fps budget, not because frames were slow
        // (6.6 ms against a 16.7 ms budget) but because they were never asked for.
        //
        // A notch asks for TRAVEL now, and the travel outlives the notch. The assertion is deliberately
        // about frames that MOVED rather than frames that ran: a grid redrawing itself unchanged is
        // exactly as staircased, and would sail through a naive frame count.
        var r = new Rig();
        r.PanAtCadence(delta: 120f, notches: 8, gapMs: 150f);

        Assert.True(
            r.FramesThatMoved > r.Frames / 2,
            $"only {r.FramesThatMoved} of {r.Frames} frames moved the grid - the scroll is a staircase");
    }

    [Fact]
    public void A_wheel_pan_runs_straight_through_a_week_boundary()
    {
        // Ten notches of 120 is 1200px, nearly two of this fixture's 648px weeks. It does not stop at
        // the seam, does not turn a page, and does not re-seat the days to the new week's first column:
        // it is one strip, and the anchor simply moves under it.
        var r = new Rig();
        r.Pan(delta: 120f, notches: 10);
        r.RunToRest();

        // 1200px is 12.96 columns, so it rests on the thirteenth, a fifth of a column further on,
        // still inside the second week, with no page turn and no re-seat to that week's first column.
        Assert.Equal(1, r.State.Week);
        Assert.Equal(13f / 7f, r.State.WeekPosition(r.State.Metrics(Screen)), 0.001f);
    }

    [Fact]
    public void The_strip_never_leaves_its_anchor_week()
    {
        // The invariant the whole draw path leans on: keep the offset inside its own week by moving the
        // anchor, and at most one seam, so at most two weeks, can ever be on screen. Break it and the
        // grid would have to draw a week it does not hold, which is the hole the old page lag could
        // punch.
        var r = new Rig();
        var m = r.State.Metrics(Screen);

        for (var i = 0; i < 40; i++)
        {
            r.Pan(delta: 100f, notches: 1);
            r.Frame(16f);
            Assert.InRange(r.State.StripX, 0f, WeekPx);
        }
        // ...and backwards, through zero and out the other side.
        for (var i = 0; i < 80; i++)
        {
            r.Pan(delta: -100f, notches: 1);
            r.Frame(16f);
            Assert.InRange(r.State.StripX, 0f, WeekPx);
        }
        r.RunToRest();
        Assert.InRange(r.State.StripX, 0f, WeekPx);

        // 40 forward, 80 back: four thousand pixels behind where it started, which is weeks ago, and
        // it comes to rest on the nearest column of them (-4000px is -43.2 columns).
        Assert.True(r.State.Week < 0, "the strip runs backwards as freely as forwards");
        Assert.Equal(-43f / 7f, r.State.WeekPosition(m), 0.001f);
    }

    [Fact]
    public void The_horizontal_wheels_sign_is_inverted()
    {
        // A WM_MOUSEHWHEEL wart, not a bug in the panning: the same raw delta that scrolls the hours
        // one way scrolls the days the other, so the owner negates it. Found by hand on a real
        // trackpad, a pan that went the wrong way, and pinned here because the negation looks exactly
        // like a mistake to anyone tidying up.
        var r = new Rig();

        r.Owner.Wheel(delta: 120f, horizontal: true, control: false, x: 350f, y: 300f);
        r.RunToRest();
        // NOT negative: the strip goes with the hand, not with the sign. (Exactly where it lands is the
        // day rule's business, what this test owns is the direction.)
        Assert.True(r.State.StripX > 0f, "a positive horizontal delta must move the days forwards");

        // The vertical wheel, for contrast, is taken exactly as reported.
        r.Owner.Wheel(delta: -120f, horizontal: false, control: false, x: 350f, y: 300f);
        Assert.Equal(120f, r.State.ScrollY, 0.01f);
    }

    [Fact]
    public void A_touchpad_pinch_persists_its_shape_once_the_wheel_falls_quiet()
    {
        // The idle timer's remaining job. A Ctrl+wheel pinch has no lift either, and the shape and
        // horizon it settles on have to reach the core exactly once, not per notch (that would push a
        // preference write across the FFI dozens of times a second) and not never.
        var r = new Rig();
        for (var i = 0; i < 4; i++)
        {
            r.Owner.Wheel(delta: 120f, horizontal: false, control: true, x: 350f, y: 300f);
        }
        Assert.Equal(0, r.ZoomsSettled); // still zooming, nothing persisted yet

        r.RunToRest();

        Assert.Equal(1, r.ZoomsSettled);
    }

    [Fact]
    public void A_horizontal_pan_settles_nothing_at_all()
    {
        // The other half of the first test, from the host's side: a pan must not be mistaken for a
        // zoom and write a shape to the core. Only Ctrl+wheel does that.
        var r = new Rig();
        r.Pan(delta: 40f, notches: 6);
        r.RunToRest();

        Assert.Equal(0, r.ZoomsSettled);
    }

    [Fact]
    public void A_zoom_at_mouse_cadence_banks_its_shape_exactly_once()
    {
        // **Why the idle window has to clear a mouse's notch gap.** Windows gives a wheel gesture no
        // phase, so silence is the only end-of-gesture signal there is, and a window shorter than the
        // gap between two notches of the SAME gesture resolves a gesture that has not finished.
        //
        // The zoom's window was 60 ms against a measured ~150 ms gap, so every notch was a finished
        // gesture. Against a real diary that was seven settles in two seconds, each one a core write
        // plus four snapshot reloads of 33-111 ms on the UI thread, mid-pinch.
        var r = new Rig();
        for (var i = 0; i < 6; i++)
        {
            r.Owner.Wheel(delta: 120f, horizontal: false, control: true, x: 350f, y: 300f);
            for (var f = 0; f < 9; f++) // ~150 ms between notches
            {
                r.Frame(16f);
            }
        }
        Assert.Equal(0, r.ZoomsSettled); // still zooming, six notches in

        r.RunToRest();

        Assert.Equal(1, r.ZoomsSettled);
    }
}
