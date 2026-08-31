// Every animation the grid runs, the flings, the wheel's glide, the day landings, and the chevrons' step.
//
// Android runs these as coroutines, and had to learn the hard way that each must live in its **own**
// job: an animation awaited inline in the gesture loop threw when the next touch preempted it, tore
// the loop down, and the grid stopped settling for the rest of the session. That was "it sits there
// forever".
//
// Windows has no coroutines here and does not want them. There is exactly **one** animation in
// flight, it is a field, and starting a new one overwrites it. A cancellation is an assignment. That
// removes the entire class of bug by construction, there is no job to cancel, and nothing to throw.
//
// The clock comes from **outside** (`Tick(dt)`), which is the other half of the design: WinUI has no
// equivalent of Compose's test clock, so owning the clock is the only way to make "a flick arrives
// while the previous turn is still mid-slide" a deterministic test rather than a thing you can only
// feel. See docs/calendar.md §9, every test that waits for the grid to settle first is testing the
// case that already worked.
//
// **What is NOT here any more: the page turn, and everything it needed.** The days are one continuous
// strip (see CalendarSurfaceState), so there is no such thing as "between two weeks", a grid showing
// Wednesday to Tuesday is showing seven days, not half of each of two pages. Every gesture now ends the
// same way: it coasts, and it lands on the day it stopped nearest.
//
// That deleted the threshold, the velocity judgement, and the banked landing this file used to carry
// across a cancelled animation so a second flick could not re-target a week the first had already won.
// A flick banks nothing: it adds speed to a strip that coasts, and the landing is a consequence of
// where the coast ended. There is no decision for a later event to disagree with, which is why the
// swallowed-swipe class of bug is gone by construction rather than guarded against (§6).
using System;

namespace Allodia.Mailcal.Calendar;

/// <summary>
/// Drives the grid's flings, glides, day landings and settles, one frame at a time.
/// </summary>
internal sealed class CalendarSurfaceDriver(CalendarSurfaceState state)
{
    /// <summary>
    /// The one animation in flight, or <c>null</c>. Starting another simply replaces it.
    /// </summary>
    private GridAnimation? _current;

    /// <summary>Whether anything is still moving, which is what the surface redraws for.</summary>
    internal bool IsAnimating => _current is not null;

    /// <summary>A finger on the glass stops whatever the grid was doing.</summary>
    /// <remarks>
    /// There is nothing to preserve across it. Every landing this file has is decided at <b>rest</b>,
    /// from where the strip actually is, so an interrupted animation leaves no decision behind for the
    /// next gesture to have to honour or discard.
    /// </remarks>
    internal void Stop() => _current = null;

    /// <summary>Coasts the hours to a stop. <paramref name="velocity"/> is the finger's, in px/s.</summary>
    internal void FlingHours(float velocity) => _current = new HourFling(velocity);

    /// <summary>
    /// Coasts the days to a stop, and rests on the day it stopped nearest.
    /// </summary>
    /// <remarks>
    /// Every released day-pan, at every zoom. It runs off the end of a week and straight into the next
    /// one without a page turn, a re-seat, or a bump, which is what a strip <i>is</i>. (The old
    /// day-scroll stopped dead at the week's edge and handed its leftover speed to the pager, which then
    /// re-seated the days to the new week's first column: a hard flick from mid-week landed you on a
    /// Monday you had not asked for, and the seam was a thing you could feel.)
    /// <para>
    /// <b>A flick decides nothing up front</b>, it coasts, and only what is left when the coast runs out
    /// is rounded to a day (<see cref="StripFling"/> chains into <see cref="SettleDay"/>). That is what
    /// lets a second flick landing mid-coast simply add its speed to the first: there is no banked target
    /// for it to disagree with, so the swallowed-swipe arithmetic that cost a week per pair of fast
    /// flicks cannot arise.
    /// </para>
    /// </remarks>
    internal void FlingStrip(float velocity) => _current = new StripFling(velocity);

    /// <summary>
    /// Moves the strip by <paramref name="dx"/> pixels of <b>wheel</b> travel, smoothly.
    /// </summary>
    /// <remarks>
    /// The wheel is not a finger: it arrives as discrete notches with no phase, so applying each one to
    /// the strip the instant it lands teleports the grid once per notch and leaves it still in between.
    /// Measured on a real mouse, 150 ms apart, that is a 6.5 Hz staircase, the "it stops at random
    /// points" report, and no amount of frame budget helps, because the frames were never asked for.
    /// <para>
    /// So a notch adds to a <i>target</i> and the strip eases toward it. Travel outlives the notch that
    /// asked for it, which keeps the grid moving between notches, keeps the surface's tick loop alive
    /// (<see cref="IsAnimating"/>) while there is distance left to cover, and turns a stream of any
    /// cadence, a mouse's sparse notches, a trackpad's dense ones, into one continuous motion.
    /// </para>
    /// </remarks>
    internal void GlideBy(float dx, SurfaceMetrics m)
    {
        if (m.WeekWidth <= 0f || dx == 0f)
        {
            return;
        }
        if (_current is StripGlide glide)
        {
            glide.Add(dx);
            return;
        }
        _current = new StripGlide(dx);
    }

    /// <summary>
    /// Brings the strip to rest on the nearest day, the one landing, for every input.
    /// </summary>
    /// <remarks>
    /// Called when a gesture is over: a coast that has run out, a wheel that has gone quiet, a capture
    /// lost mid-swipe. It moves the grid by at most half a day, which is small enough to read as the
    /// grid settling rather than as it overruling the user (<see cref="CalendarSurfaceState.NearestDay"/>).
    /// </remarks>
    internal void SettleDay(SurfaceMetrics m) => SnapTo(state.NearestDay(m), m);

    /// <summary>
    /// Steps the strip by <paramref name="days"/>, what the header's <c>&lt;</c> and <c>&gt;</c> do.
    /// </summary>
    /// <remarks>
    /// By the <b>visible span</b> (the day zoom moves a day, the three-day zoom three, the week a
    /// week), so a chevron never skips a day the user could not see. It steps from wherever the strip
    /// is, and does not re-align: click <c>&gt;</c> on a Wednesday-to-Tuesday view and you get the next
    /// Wednesday to Tuesday. Anything else would move the grid by an amount the user did not ask for.
    /// </remarks>
    internal void SlideDays(int days, SurfaceMetrics m) =>
        SnapTo(state.WeekPosition(m) + ((float)days / CalendarUnits.DaysInWeek), m);

    /// <summary>Springs the strip to <paramref name="targetWeeks"/>, the one landing this file has.</summary>
    private void SnapTo(float targetWeeks, SurfaceMetrics m)
    {
        if (m.WeekWidth <= 0f)
        {
            return;
        }
        var displacement = (state.WeekPosition(m) - targetWeeks) * m.WeekWidth;
        _current = new StripSnap(targetWeeks, displacement);
    }

    /// <summary>
    /// Advances whatever is in flight by <paramref name="dt"/> seconds.
    /// </summary>
    /// <remarks>
    /// The slot is only cleared when the animation that just finished is still the one sitting in it,
    /// an animation is allowed to replace itself, and clearing unconditionally would throw away
    /// whatever it started.
    /// </remarks>
    internal void Tick(float dt, SurfaceMetrics m)
    {
        if (_current is not { } animation || dt <= 0f)
        {
            return;
        }
        if (!animation.Step(dt, state, m, this) && ReferenceEquals(_current, animation))
        {
            _current = null;
        }
    }

    /// <summary>One animation. <see cref="Step"/> returns <c>false</c> when it is finished.</summary>
    private abstract class GridAnimation
    {
        internal abstract bool Step(
            float dt,
            CalendarSurfaceState state,
            SurfaceMetrics m,
            CalendarSurfaceDriver driver);
    }

    /// <summary>
    /// The strip landing, measured in <b>weeks</b> (fractionally, so a day is a seventh), sprung in
    /// pixels.
    /// </summary>
    /// <remarks>
    /// The target is a week fraction and not a pixel on purpose: a pixel means a different day at every
    /// zoom, and the anchor week moves under the animation every time the strip crosses a boundary. The
    /// week coordinate survives both.
    /// </remarks>
    private sealed class StripSnap(float targetWeeks, float displacement) : GridAnimation
    {
        private float _displacement = displacement;
        private float _velocity;

        internal override bool Step(
            float dt,
            CalendarSurfaceState state,
            SurfaceMetrics m,
            CalendarSurfaceDriver driver)
        {
            if (m.WeekWidth <= 0f)
            {
                return false;
            }
            (_displacement, _velocity) = CalendarSpring.Step(_displacement, _velocity, dt);
            if (CalendarSpring.AtRest(_displacement, _velocity))
            {
                // Land exactly. A column resting a third of a pixel out is a column resting out.
                state.ScrollToWeeks(targetWeeks, m);
                return false;
            }
            state.ScrollToWeeks(targetWeeks + (_displacement / m.WeekWidth), m);
            return true;
        }
    }

    /// <summary>The hours coasting to a stop.</summary>
    /// <remarks>
    /// The content moves opposite the finger: flick up, the day scrolls down. Off the end of the day
    /// it <i>stops</i>, rather than gliding on against a wall.
    /// </remarks>
    private sealed class HourFling(float fingerVelocity) : GridAnimation
    {
        private float _velocity = -fingerVelocity;

        internal override bool Step(
            float dt,
            CalendarSurfaceState state,
            SurfaceMetrics m,
            CalendarSurfaceDriver driver)
        {
            var target = state.ScrollY + (_velocity * dt);
            state.ScrollTo(target, m);
            if (MathF.Abs(state.ScrollY - target) > 0.01f)
            {
                // Cut short by the top or the bottom of the day.
                return false;
            }
            _velocity = CalendarDecay.Decay(_velocity, dt);
            return !CalendarDecay.AtRest(_velocity);
        }
    }

    /// <summary>
    /// The day strip coasting to a stop, through the weeks, and past them, then resting on a day.
    /// </summary>
    /// <remarks>
    /// It has no end to run into. A hard flick carries across as many weeks as its speed is worth and
    /// stops wherever it runs out, which is what makes the calendar feel like one long strip of days
    /// rather than a stack of pages with walls between them. Only when the speed is gone is the
    /// remainder rounded to a day, by handing over to <see cref="SettleDay"/>, so the landing is a
    /// consequence of where the coast ended, never a target the flick committed to up front.
    /// </remarks>
    private sealed class StripFling(float fingerVelocity) : GridAnimation
    {
        private float _velocity = -fingerVelocity;

        internal override bool Step(
            float dt,
            CalendarSurfaceState state,
            SurfaceMetrics m,
            CalendarSurfaceDriver driver)
        {
            state.StripTo(state.StripX + (_velocity * dt), m);
            _velocity = CalendarDecay.Decay(_velocity, dt);
            if (!CalendarDecay.AtRest(_velocity))
            {
                return true;
            }
            // Hands the slot to the settle, which is why Tick only clears it when the finished
            // animation is still the one sitting there.
            driver.SettleDay(m);
            return false;
        }
    }

    /// <summary>
    /// The strip easing toward the travel a wheel has asked for.
    /// </summary>
    /// <remarks>
    /// Exponential, not linear: a fixed <i>fraction</i> of what is left each frame, so the strip is
    /// quickest exactly when it is furthest behind the input and never overshoots. <see cref="Add"/>
    /// folds a new notch into the same journey rather than starting another, which is what makes a
    /// stream of notches one motion instead of a queue of jumps.
    /// <para>
    /// It deliberately does <b>not</b> land on a day when it drains. The wheel has no lift, so a drained
    /// glide means only "no notch for a few frames", which happens constantly mid-scroll; rounding there
    /// would tug the grid backwards under an unfinished gesture, the snap-back this file's header is
    /// about, in miniature. The owner's idle timer decides the gesture is over and calls
    /// <see cref="SettleDay"/> then.
    /// </para>
    /// </remarks>
    private sealed class StripGlide(float dx) : GridAnimation
    {
        /// <summary>Seconds for the strip to cover ~63% of the distance outstanding.</summary>
        private const float Tau = 0.06f;

        /// <summary>Below this many pixels outstanding, the journey is over.</summary>
        private const float RestPx = 0.5f;

        private float _remaining = dx;

        /// <summary>Folds another notch's travel into the journey already under way.</summary>
        internal void Add(float more) => _remaining += more;

        internal override bool Step(
            float dt,
            CalendarSurfaceState state,
            SurfaceMetrics m,
            CalendarSurfaceDriver driver)
        {
            if (MathF.Abs(_remaining) <= RestPx)
            {
                state.PanX(_remaining, m);
                _remaining = 0f;
                return false;
            }
            var step = _remaining * (1f - MathF.Exp(-dt / Tau));
            _remaining -= step;
            state.PanX(step, m);
            return true;
        }
    }
}
