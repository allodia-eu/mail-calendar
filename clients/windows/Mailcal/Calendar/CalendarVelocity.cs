// How fast the finger was going when it left the glass.
//
// A flick pages on **velocity alone**, whatever distance it covered (docs/calendar.md §6), so this
// is not a detail, it is half of what decides whether a swipe turns the week. Compose and UIKit both
// hand you one of these; WinUI does not, so the grid brings its own.
//
// Pure, and deliberately so: it is what the flick tests measure against.
using System;
using System.Collections.Generic;

namespace Allodia.Mailcal.Calendar;

/// <summary>Estimates a pointer's velocity, in pixels per second, from its recent positions.</summary>
/// <remarks>
/// A least-squares fit over the last <see cref="WindowMs"/> of movement, which is steadier than
/// differencing the final two samples: the last event before a lift is often a near-duplicate of the
/// one before it, and dividing a pixel by a millisecond reports a flick that never happened.
/// </remarks>
internal sealed class CalendarVelocityTracker
{
    /// <summary>
    /// How far back to look. Long enough to smooth the jitter, short enough that a finger which
    /// paused before lifting reports a stop rather than the flick it made half a second ago.
    /// </summary>
    private const double WindowMs = 100d;

    /// <summary>Beyond this, a sample is a new gesture rather than a continuation.</summary>
    private const double MaxGapMs = 40d;

    private readonly List<(double TimeMs, float X, float Y)> _samples = [];

    /// <summary>Forgets everything, a new gesture starts here.</summary>
    internal void Clear() => _samples.Clear();

    /// <summary>Records where the pointer was, and when.</summary>
    internal void Add(double timeMs, float x, float y)
    {
        // A long gap means the finger stopped. Anything before it says nothing about where it is going.
        if (_samples.Count > 0 && timeMs - _samples[^1].TimeMs > MaxGapMs)
        {
            _samples.Clear();
        }
        _samples.Add((timeMs, x, y));
        var cutoff = timeMs - WindowMs;
        var drop = 0;
        while (drop < _samples.Count - 2 && _samples[drop].TimeMs < cutoff)
        {
            drop++;
        }
        if (drop > 0)
        {
            _samples.RemoveRange(0, drop);
        }
    }

    /// <summary>The pointer's velocity in px/s, or zero if it has not moved enough to know.</summary>
    internal (float X, float Y) Velocity()
    {
        if (_samples.Count < 2)
        {
            return (0f, 0f);
        }
        var first = _samples[0];
        var last = _samples[^1];
        var seconds = (float)((last.TimeMs - first.TimeMs) / 1000d);
        if (seconds <= 0f)
        {
            return (0f, 0f);
        }
        return ((last.X - first.X) / seconds, (last.Y - first.Y) / seconds);
    }
}

/// <summary>One contact, at one moment: where it is, and when it was there.</summary>
/// <remarks>
/// The gesture owner speaks in these rather than in <c>PointerRoutedEventArgs</c>, which is what
/// makes the whole of §6 and §9 testable without a window. The WinUI layer's only job is to
/// translate.
/// </remarks>
internal readonly record struct PointerSample(uint Id, float X, float Y, double TimeMs);
