// How a swipe turns the page, and how a fling coasts to a stop, as arithmetic, not as a framework.
//
// Two numbers decide whether a calendar feels "unbreakable", and only one of them is the one people
// reach for. Measured on Android off screen recordings of Samsung Calendar and ours on the same
// phone, driven by the same hand:
//
//                       page turn takes      peak speed
//     ours (before)     0.32 – 0.50 s        ~5 pages/s
//     Samsung           0.02 – 0.15 s        20 – 28 pages/s
//
// The threshold, how far you must drag before it commits, was NOT the difference. At 20+ pages a
// second Samsung is committing on **velocity** anyway, so the finger's travel is irrelevant to it.
// The difference was the **settle**: theirs is over in a tenth of a second, ours took most of half a
// second. That is what makes rapid swipes feel like fighting the app, each new flick lands on a
// grid still gliding from the last one. See docs/calendar.md §6.
//
// Both curves are closed-form and stepped by a caller-supplied `dt`. That is deliberate: WinUI has
// no equivalent of Compose's test clock, so the only way to make "a gesture arrives while the
// previous animation is still running" a deterministic test is to own the clock ourselves.
using System;

namespace Allodia.Mailcal.Calendar;

/// <summary>The settle: a critically damped spring, solved rather than integrated.</summary>
internal static class CalendarSpring
{
    /// <summary>
    /// How briskly the page lands.
    /// </summary>
    /// <remarks>
    /// Tuned by measurement on Android, not by taste: a medium spring got a page turn down to 0.23 s,
    /// which was still twice Samsung's. This sits between medium and high and lands the page in about
    /// a tenth of a second, their number.
    /// </remarks>
    internal const float Stiffness = 4000f;

    /// <summary>
    /// Below this displacement (px) and velocity (px/s), the spring is done.
    /// </summary>
    /// <remarks>
    /// A closed-form critically damped spring approaches its target asymptotically and never actually
    /// arrives, so something has to call it. Sub-pixel is invisible.
    /// </remarks>
    internal const float RestDisplacement = 0.5f;

    /// <summary>The velocity below which the spring is considered stopped, in px/s.</summary>
    internal const float RestVelocity = 8f;

    /// <summary>The undamped natural frequency, for unit mass.</summary>
    private static readonly float Omega = MathF.Sqrt(Stiffness);

    /// <summary>
    /// Advances a spring pulling <paramref name="displacement"/> towards zero by
    /// <paramref name="dt"/> seconds.
    /// </summary>
    /// <remarks>
    /// <b>Critically damped, no bounce, is deliberate, and not a matter of taste.</b> A week that
    /// springs past its own column and comes back is a grid whose day columns visibly overshoot the
    /// headings above them. On a dense week that reads as a glitch, not as polish.
    /// <para>
    /// For a critically damped system, <c>x(t) = (c₁ + c₂t)·e^(−ωt)</c>, with <c>c₁</c> the initial
    /// displacement and <c>c₂ = v₀ + ω·c₁</c>. Closed-form rather than integrated, so a long frame
    /// cannot make the spring behave differently from a short one.
    /// </para>
    /// </remarks>
    internal static (float Displacement, float Velocity) Step(float displacement, float velocity, float dt)
    {
        var c1 = displacement;
        var c2 = velocity + (Omega * displacement);
        var decay = MathF.Exp(-Omega * dt);
        var linear = c1 + (c2 * dt);
        return (linear * decay, (c2 - (Omega * linear)) * decay);
    }

    /// <summary>Whether the spring has arrived, near enough that nobody could see the difference.</summary>
    internal static bool AtRest(float displacement, float velocity) =>
        MathF.Abs(displacement) < RestDisplacement && MathF.Abs(velocity) < RestVelocity;
}

/// <summary>The fling: exponential decay, the curve a thrown scroll coasts along.</summary>
internal static class CalendarDecay
{
    /// <summary>
    /// How fast a thrown scroll bleeds off speed, per second.
    /// </summary>
    /// <remarks>
    /// The same constant Compose's <c>exponentialDecay</c> uses, so a flung day strip coasts the same
    /// distance on both clients. Velocity is <c>v₀·e^(−friction·t)</c>, so a fling travels
    /// <c>v₀ / friction</c> pixels in total.
    /// <para>
    /// This is a <b>feel</b> constant, and feel is not testable from a chair (docs/calendar.md §9). It
    /// is the first thing to re-tune against the real machine, not against a unit test.
    /// </para>
    /// </remarks>
    internal const float Friction = 4.2f;

    /// <summary>Below this speed (px/s) the fling has stopped; carrying on just burns frames.</summary>
    internal const float RestVelocity = 1f;

    /// <summary>The velocity remaining after <paramref name="dt"/> seconds of coasting.</summary>
    internal static float Decay(float velocity, float dt) => velocity * MathF.Exp(-Friction * dt);

    /// <summary>Whether the fling has run out of speed.</summary>
    internal static bool AtRest(float velocity) => MathF.Abs(velocity) <= RestVelocity;
}
