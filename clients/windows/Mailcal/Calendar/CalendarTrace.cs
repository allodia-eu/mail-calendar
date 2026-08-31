// What the grid decided each finger was, and what each frame cost, counts and durations, never
// content.
//
// Off in every build unless asked for, and it costs one cached boolean. It reports what the single
// gesture owner DECIDED, pan / page / zoom / tap, which is how you catch a pinch being misread as a
// pan by a real hand, and it is what proves the zoom no longer pans the grid.
//
// It found Android's shaper bug, too: a pinch frame costing 3.4x a swipe frame while drawing HALF the
// blocks is not a performance problem, it is a clue.
//
// **It never logs a title, a time, or an attendee.** This runs against real diaries, and
// docs/logging.md's never-log-content rule does not bend for convenience.
using System;
using System.Diagnostics;
using Allodia.Mailcal.Services;

namespace Allodia.Mailcal.Calendar;

/// <summary>The grid's diagnostic counters. Enabled by <c>MAILCAL_CAL_TRACE=1</c>.</summary>
internal sealed class CalendarTrace
{
    /// <summary>How often to report, in frames. Roughly once a second while something is moving.</summary>
    private const int ReportEvery = 60;

    private static readonly bool Enabled =
        Environment.GetEnvironmentVariable("MAILCAL_CAL_TRACE") is "1" or "true";

    private readonly Stopwatch _frame = new();
    private int _frames;
    private int _drawn;
    private int _culled;
    private int _painted;
    private int _turned;
    private double _frameMsTotal;
    private double _frameMsWorst;

    /// <summary>Whether the trace is on. Check it before doing any work for it.</summary>
    internal static bool On => Enabled;

    /// <summary>A page was rebuilt from the core, the thing a zoom must never cause.</summary>
    internal void Painted()
    {
        if (Enabled)
        {
            _painted++;
        }
    }

    /// <summary>A week was banked.</summary>
    internal void Turned()
    {
        if (Enabled)
        {
            _turned++;
        }
    }

    /// <summary>A block was drawn.</summary>
    internal void Drew()
    {
        if (Enabled)
        {
            _drawn++;
        }
    }

    /// <summary>A block was culled, outside the viewport, so it cost one comparison.</summary>
    internal void Culled()
    {
        if (Enabled)
        {
            _culled++;
        }
    }

    /// <summary>What the single owner decided this gesture was.</summary>
    internal static void Gesture(GestureMode mode)
    {
        if (Enabled)
        {
            Log.Info($"cal gesture={mode}");
        }
    }

    internal void FrameBegin()
    {
        if (Enabled)
        {
            _frame.Restart();
        }
    }

    /// <summary>
    /// Ends a frame and, once a second, reports what the last sixty cost.
    /// </summary>
    /// <remarks>
    /// The <b>worst</b> frame is reported alongside the mean, deliberately. A mean hides exactly the
    /// thing the eye sees: the grid is judged on the frames it MISSES during motion, not on the ones
    /// it makes (§7).
    /// </remarks>
    internal void FrameEnd(int shaped)
    {
        if (!Enabled)
        {
            return;
        }
        _frame.Stop();
        var ms = _frame.Elapsed.TotalMilliseconds;
        _frames++;
        _frameMsTotal += ms;
        _frameMsWorst = Math.Max(_frameMsWorst, ms);

        if (_frames < ReportEvery)
        {
            return;
        }
        Log.Info(
            $"cal frames={_frames} mean={_frameMsTotal / _frames:F2}ms worst={_frameMsWorst:F2}ms " +
            $"blocks={_drawn} culled={_culled} shaped={shaped} pages={_painted} turns={_turned}");
        _frames = 0;
        _drawn = 0;
        _culled = 0;
        _painted = 0;
        _turned = 0;
        _frameMsTotal = 0;
        _frameMsWorst = 0;
    }
}
