// The wheel, a mouse, and a precision touchpad's digested two-finger pan.
//
// Split from CalendarGestureOwner.cs to stay under the repo's 500-line file cap; it is the same class
// (the primary-constructor parameters state/driver/viewport and the wheel fields are all in scope
// here). The seam is real, though: touch delivers raw contacts the owner tracks itself, while the
// touchpad never does, Windows digests its two-finger pan into wheel messages, so this is a genuinely
// different input path onto the same single owner (§6).
//
// **A notch asks for travel; it does not move the grid itself.**
//
// A wheel has no "up", Windows gives a touchpad pan no phase at all, so a lift, the OS's inertia and
// an active pan are the same burst of messages. Applying each notch to the strip the moment it lands
// therefore teleports the grid once per notch and leaves it perfectly still in between. Measured on a
// real mouse at 150 ms between notches: 24 notches over 3.8 s drew ~16–31 fps, at a mean frame cost of
// 6.6 ms against a 16.7 ms budget. The frames were not slow, they were never asked for, a 6.5 Hz
// staircase, reported as "the scroll stops at random points".
//
// So a notch adds to a target and the driver eases the strip toward it (CalendarSurfaceDriver.GlideBy).
// The travel outlives the notch that asked for it, which keeps the grid moving between notches and
// keeps the surface's tick loop alive while there is distance left to cover. The same code turns a
// mouse's sparse notches and a trackpad's dense stream into one continuous motion.
//
// **What the idle timer is for, and what it must not do.** Silence is still the only end-of-gesture
// signal there is, so it decides two things: a Ctrl+wheel zoom persists its shape exactly once, and a
// pan comes to rest on a day. Both windows are deliberately longer than a mouse's gap between notches
// A timer that fires *mid-scroll* resolves a gesture that has not finished, which is exactly the
// rubber-band that cost this file its previous design. The zoom's window is the longer of the two
// because banking it is the expensive one: it writes through to the core.
//
// The one thing here that is still a genuine Windows quirk, and is pinned by CalendarWheelTests:
// the horizontal wheel's sign is inverted relative to the vertical one (a WM_MOUSEHWHEEL wart).
using System;

namespace Allodia.Mailcal.Calendar;

internal sealed partial class CalendarGestureOwner
{
    /// <summary>
    /// How long the wheel must go quiet before a <b>pan</b> counts as over and lands on a day.
    /// </summary>
    /// <remarks>
    /// Longer than the gap between a mouse's notches (~150 ms measured), so a steady scroll is never
    /// interrupted by its own landing; short enough that letting go feels answered.
    /// </remarks>
    private const float PanSettleSeconds = 0.25f;

    /// <summary>
    /// How long the wheel must go quiet before a <b>zoom</b> banks its shape to the core.
    /// </summary>
    /// <remarks>
    /// This one was 60 ms, and that is far shorter than the gap between two notches of a mouse wheel,
    /// so every notch was read as a finished gesture and banked. Measured against a real diary: seven
    /// settles in two seconds, each a core write plus four snapshot reloads of 33–111 ms on the UI
    /// thread, mid-pinch. A page rebuilt from the core is the one thing a zoom must never cause (§7).
    /// </remarks>
    private const float ZoomSettleSeconds = 0.35f;

    /// <summary>How long the current kind of wheel gesture waits before it is called over.</summary>
    private float SettleSeconds => _wheelZoomed ? ZoomSettleSeconds : PanSettleSeconds;

    /// <summary>Whether a wheel gesture is still open, and so still needs ticking to be able to end.</summary>
    internal bool NeedsTick => _wheelIdle < SettleSeconds;

    /// <summary>
    /// One wheel notch, from a mouse, or from a precision touchpad's digested two-finger gesture.
    /// </summary>
    /// <remarks>
    /// <paramref name="delta"/> is in wheel units (120 per notch). <paramref name="control"/> means
    /// zoom, and on a touchpad that <i>is</i> the pinch: Windows never hands an app the touchpad's raw
    /// contacts, so the pinch arrives as a scalar and only the hours can move.
    /// <paramref name="horizontal"/> means the days.
    /// </remarks>
    internal void Wheel(float delta, bool horizontal, bool control, float x, float y)
    {
        if (delta == 0f)
        {
            return;
        }
        var live = viewport();
        var metrics = state.Metrics(live);
        _wheelIdle = 0f;
        _wheelOpen = true;

        if (control)
        {
            // A scalar pinch: the hours, anchored under the cursor. The day axis cannot follow, there
            // is no second component to follow it with.
            var factor = MathF.Exp(delta / 120f * WheelZoomPerNotch);
            if (!_wheelZoomed)
            {
                state.BeginZoom(metrics);
            }
            state.Pinch(
                xScale: 1f,
                yScale: factor,
                focusX: x - metrics.Gutter,
                focusY: y - metrics.ContentTop,
                viewport: live);
            _wheelZoomed = true;
            return;
        }

        if (horizontal)
        {
            // **The horizontal wheel's sign is inverted relative to the vertical one.** This is a
            // long-standing Windows quirk of `WM_MOUSEHWHEEL` reporting, not a bug in the panning:
            // vertical goes straight to PanY and reads correctly by hand, but the same raw delta fed
            // to PanX scrolls the days the opposite way from the touchpad gesture. So negate it.
            // (Touch is unaffected: it never uses the wheel, it delivers real contacts.)
            driver.GlideBy(-delta, metrics);
            return;
        }

        state.PanY(delta, metrics);
    }

    /// <summary>
    /// Advances the wheel's idle timer, so a wheel gesture can end.
    /// </summary>
    /// <remarks>
    /// Called from the same render tick that drives the animations. Neither a wheel pan nor a Ctrl+wheel
    /// zoom has a lift to wait for, so silence is the only signal available for either.
    /// </remarks>
    internal void Tick(float dt)
    {
        if (!NeedsTick)
        {
            return;
        }
        _wheelIdle += dt;
        if (_wheelIdle >= SettleSeconds)
        {
            EndWheelGesture(settlePan: true);
        }
    }

    /// <summary>
    /// Closes an open wheel gesture: bank a settled zoom, or bring a pan to rest on a day.
    /// </summary>
    /// <param name="settlePan">
    /// Whether a pan should land. False when a <b>finger</b> is taking the grid over: the touch gesture
    /// owns the strip from here, and starting a settle underneath it would animate against the hand.
    /// A zoom is still banked either way, it writes to the core, and that must not be lost.
    /// </param>
    private void EndWheelGesture(bool settlePan)
    {
        // "Open" means a wheel gesture has started and not yet been resolved, a dedicated flag, NOT
        // `_wheelIdle < SettleSeconds`. That check was self-contradicting: Tick calls this the instant
        // the wheel goes idle (`_wheelIdle >= SettleSeconds`), so it was false exactly when a real
        // gesture needed finishing.
        var wasOpen = _wheelOpen;
        var wasZoom = _wheelZoomed;
        _wheelOpen = false;
        _wheelZoomed = false;
        _wheelIdle = float.PositiveInfinity;

        if (!wasOpen)
        {
            return;
        }
        if (wasZoom)
        {
            onZoomSettled();
            return;
        }
        if (settlePan)
        {
            driver.SettleDay(state.Metrics(viewport()));
        }
    }
}
