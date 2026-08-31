// One gesture, one owner.
//
// This is the file the calendar contract's §6 asks for. Every pointer event on the time grid arrives
// here, and here alone: no ScrollViewer, no nested scroller, no separate pinch recogniser reading
// the same finger behind our back. The owner decides, pan, page, or zoom, and drives all three
// itself. Android learned that four handlers over one pointer stream is not a tuning problem and is
// not fixable from inside; Windows does not get to relearn it.
//
// **Windows adds a second input channel Android does not have, and it belongs to the same owner.**
// A precision touchpad does not give an app its raw contacts: Windows digests them and delivers
// two-finger pan as *wheel* messages, and pinch as Ctrl+wheel. A mouse sends the same wheel. If the
// wheel went to a ScrollViewer while touch came here, we would have rebuilt the exact
// four-handlers-one-finger bug in a Windows costume, so it does not. It comes here.
//
// The consequence is a real capability difference, stated rather than hidden: a touchSCREEN pinch has
// two true contacts and zooms both axes, diagonally. A touchPAD pinch is a scalar Ctrl+wheel, so it
// zooms the hours only. That is the same shortfall macOS has, for the same reason, and it is logged
// in docs/calendar.md's "Known gaps".
using System;
using System.Collections.Generic;

namespace Allodia.Mailcal.Calendar;

/// <summary>What the finger turned out to be. Decided once, on the first movement that means anything.</summary>
internal enum GestureMode
{
    /// <summary>Nothing yet, it may still become a tap.</summary>
    Undecided,

    /// <summary>Sideways: the strip of days, and the weeks it runs through.</summary>
    PanDays,

    /// <summary>Up and down: the hours.</summary>
    PanHours,

    /// <summary>Two fingers, genuinely spreading.</summary>
    Zoom,
}

/// <summary>
/// The whole pointer contract of the time grid.
/// </summary>
/// <remarks>
/// Fed abstract <see cref="PointerSample"/>s and wheel notches rather than WinUI events, so every
/// rule in docs/calendar.md §6, including the one that only reproduces when a gesture arrives
/// <i>while the previous gesture's animation is still running</i>, is a headless unit test.
/// </remarks>
internal sealed partial class CalendarGestureOwner(
    CalendarSurfaceState state,
    CalendarSurfaceDriver driver,
    Func<SurfaceViewport> viewport,
    float touchSlop,
    Action onZoomSettled,
    Action<float, float> onTap)
{
    /// <summary>
    /// How far apart two fingers must be <b>on an axis</b> before that axis's scale means anything.
    /// </summary>
    /// <remarks>
    /// This is what keeps the axes independent without forbidding diagonals. Fingers spread purely
    /// sideways sit at almost the same height, so their vertical spread is a few noisy pixels, and
    /// dividing by it would produce a wild factor and lurch the hours about while the user was only
    /// asking for more days. Below this, an axis reports "no change" rather than a number it cannot
    /// know. Spread them at an angle and <i>both</i> spreads are real, so both axes zoom.
    /// </remarks>
    private const float MinSpreadPx = 48f;

    /// <summary>
    /// How far the fingers' spread must <b>change</b> before the gesture is a pinch and not a
    /// two-finger swipe.
    /// </summary>
    /// <remarks>
    /// Two fingers 200px apart that wobble by one pixel give a scale of 1.005, which is not 1.
    /// Without a slop, <i>any</i> two-finger contact read as a pinch, and that is how the swipe got
    /// stolen.
    /// </remarks>
    private const float PinchSlopPx = 10f;

    /// <summary>How much of an hour one Ctrl+wheel notch zooms. A wheel notch is 120 units.</summary>
    private const float WheelZoomPerNotch = 0.12f;

    /// <summary>Where each contact is now.</summary>
    private readonly Dictionary<uint, PointerSample> _pointers = [];

    /// <summary>
    /// Where each contact was on the previous event, which is the only thing a pinch can measure
    /// against. Snapshotted before each move lands, never derived after the fact.
    /// </summary>
    private readonly Dictionary<uint, PointerSample> _previous = [];

    private readonly CalendarVelocityTracker _tracker = new();

    private GestureMode _mode = GestureMode.Undecided;
    private float _travelX;
    private float _travelY;
    private float _centroidX;
    private float _centroidY;
    private int _pointerCount;
    private bool _hasSpreadStart;
    private float _spreadStartX;
    private float _spreadStartY;

    private float _downX;
    private float _downY;

    private float _wheelIdle = float.PositiveInfinity;
    private bool _wheelZoomed;

    /// <summary>Whether a wheel gesture has started and not yet been closed.</summary>
    private bool _wheelOpen;

    /// <summary>Whether a contact is currently down.</summary>
    internal bool IsTracking => _pointers.Count > 0;

    /// <summary>What the owner decided this gesture was, for the trace, and for the tests.</summary>
    internal GestureMode Mode => _mode;

    // ---- Touch and pen ---------------------------------------------------------------------------

    /// <summary>A contact went down.</summary>
    internal void PointerDown(PointerSample p)
    {
        if (_pointers.Count == 0)
        {
            // A finger on the glass stops whatever the grid was doing, and closes whatever the wheel
            // was in the middle of, so a zoom the touchpad never told us had ended is persisted before
            // the new gesture starts changing it.
            driver.Stop();
            EndWheelGesture(settlePan: false);
            _tracker.Clear();
            _mode = GestureMode.Undecided;
            _travelX = 0f;
            _travelY = 0f;
            _downX = p.X;
            _downY = p.Y;
            _hasSpreadStart = false;
        }
        _pointers[p.Id] = p;
        SnapshotPrevious();
        Recentre(p.TimeMs);
    }

    /// <summary>A contact moved. This is where pan, page and zoom are decided and driven.</summary>
    internal void PointerMoved(PointerSample p)
    {
        if (!_pointers.ContainsKey(p.Id))
        {
            return;
        }

        // The pinch needs to know how far apart the fingers were a moment ago, so the previous frame
        // is captured BEFORE this event lands on top of it.
        SnapshotPrevious();
        _pointers[p.Id] = p;

        var metrics = state.Metrics(viewport());
        var pressed = Pressed();

        // A pinch, once the fingers have genuinely spread. Two fingers merely travelling together are
        // a SWIPE, and reading them as a zoom is how the swipe got stolen.
        if (pressed.Count >= 2 && _mode != GestureMode.Zoom)
        {
            var (sx, sy) = Spread(pressed[0], pressed[1]);
            if (!_hasSpreadStart)
            {
                (_spreadStartX, _spreadStartY) = (sx, sy);
                _hasSpreadStart = true;
            }
            if (MathF.Abs(sx - _spreadStartX) > PinchSlopPx || MathF.Abs(sy - _spreadStartY) > PinchSlopPx)
            {
                _mode = GestureMode.Zoom;
                // Pin the width the labels are shaped against, for as long as the fingers are down.
                state.BeginZoom(metrics);
            }
        }
        else if (pressed.Count < 2)
        {
            _hasSpreadStart = false;
        }

        if (_mode == GestureMode.Zoom && pressed.Count >= 2)
        {
            Zoom(pressed[0], pressed[1], metrics);
            Recentre(p.TimeMs);
            return;
        }

        // The centroid, so adding or lifting a finger mid-drag does not lurch the grid: on the frame
        // the pointer count changes there is no delta at all, and the next one measures from the new
        // middle rather than jumping to it.
        var (cx, cy) = Centroid(pressed);
        var dx = _pointerCount == pressed.Count ? cx - _centroidX : 0f;
        var dy = _pointerCount == pressed.Count ? cy - _centroidY : 0f;
        _centroidX = cx;
        _centroidY = cy;
        _pointerCount = pressed.Count;
        _tracker.Add(p.TimeMs, cx, cy);

        if (_mode == GestureMode.Undecided)
        {
            _travelX += dx;
            _travelY += dy;
            // One axis, decided once. The hours and the days are separate scrolls to the user's hand,
            // and a drag that did both at once would turn the week while they were reading down a day.
            if (MathF.Abs(_travelX) > touchSlop || MathF.Abs(_travelY) > touchSlop)
            {
                _mode = MathF.Abs(_travelX) > MathF.Abs(_travelY)
                    ? GestureMode.PanDays
                    : GestureMode.PanHours;
            }
        }

        switch (_mode)
        {
            case GestureMode.PanDays:
                state.PanX(dx, metrics);
                break;
            case GestureMode.PanHours:
                state.PanY(dy, metrics);
                break;
            default:
                break;
        }
    }

    /// <summary>A contact lifted. The last one to go decides what the gesture was.</summary>
    internal void PointerUp(PointerSample p)
    {
        _pointers.Remove(p.Id);
        SnapshotPrevious();
        if (_pointers.Count > 0)
        {
            // Still fingers down: re-seat the centroid so the remaining one does not lurch the grid.
            Recentre(p.TimeMs);
            return;
        }

        var metrics = state.Metrics(viewport());
        var (vx, vy) = _tracker.Velocity();

        switch (_mode)
        {
            case GestureMode.Zoom:
                onZoomSettled();
                break;
            case GestureMode.PanHours:
                driver.FlingHours(vy);
                break;
            // One rule, at every zoom: it coasts, and rests on the day it stopped nearest. There is no
            // page to turn, the strip is continuous, so a release is a release however many columns
            // happen to be on screen.
            case GestureMode.PanDays:
                driver.FlingStrip(vx);
                break;
            default:
                // Never moved: a tap.
                onTap(_downX, _downY);
                break;
        }
        _mode = GestureMode.Undecided;
        _pointerCount = 0;
    }

    /// <summary>
    /// The gesture was taken away, pointer capture lost, a system dialog, the window deactivating.
    /// </summary>
    /// <remarks>
    /// None of the release path ran, so the landing has to be done for it, otherwise the grid sits
    /// resting between two days for as long as the user looks at it. It lands from where the strip
    /// actually is, which is all any landing does now.
    /// </remarks>
    internal void PointerCancelled()
    {
        _pointers.Clear();
        _previous.Clear();
        _pointerCount = 0;
        var wasZooming = _mode == GestureMode.Zoom;
        var wasPanning = _mode == GestureMode.PanDays;
        _mode = GestureMode.Undecided;
        if (wasZooming)
        {
            onZoomSettled();
        }
        if (wasPanning)
        {
            driver.SettleDay(state.Metrics(viewport()));
        }
    }

    // The wheel, a mouse, and a precision touchpad's digested two-finger pan, lives in
    // CalendarGestureOwner.Wheel.cs. Split out only to stay under the 500-line file cap; it is the
    // same class, and it reaches the same state, driver and callbacks.

    // ---- Arithmetic --------------------------------------------------------------------------------

    private List<PointerSample> Pressed()
    {
        var pressed = new List<PointerSample>(_pointers.Count);
        foreach (var p in _pointers.Values)
        {
            pressed.Add(p);
        }
        // A stable order, so "the first two fingers" means the same thing from one event to the next,
        // a dictionary's order is not a promise, and a pinch that swapped its two contacts mid-gesture
        // would read the spread backwards for one frame and lurch.
        pressed.Sort(static (a, b) => a.Id.CompareTo(b.Id));
        return pressed;
    }

    private void SnapshotPrevious()
    {
        _previous.Clear();
        foreach (var kv in _pointers)
        {
            _previous[kv.Key] = kv.Value;
        }
    }

    private void Recentre(double timeMs)
    {
        var pressed = Pressed();
        if (pressed.Count == 0)
        {
            return;
        }
        (_centroidX, _centroidY) = Centroid(pressed);
        _pointerCount = pressed.Count;
        _tracker.Add(timeMs, _centroidX, _centroidY);
    }

    private static (float X, float Y) Centroid(List<PointerSample> pressed)
    {
        var sx = 0f;
        var sy = 0f;
        foreach (var p in pressed)
        {
            sx += p.X;
            sy += p.Y;
        }
        return (sx / pressed.Count, sy / pressed.Count);
    }

    private static (float X, float Y) Spread(PointerSample a, PointerSample b) =>
        (MathF.Abs(a.X - b.X), MathF.Abs(a.Y - b.Y));

    /// <summary>
    /// One frame of a pinch, both axes, each by its own component of the spread.
    /// </summary>
    /// <remarks>
    /// The focal point is handed to the state in <b>content</b> coordinates (past the hour ruler,
    /// below the banner), which is the frame the scroll offsets live in.
    /// </remarks>
    private void Zoom(PointerSample a, PointerSample b, SurfaceMetrics metrics)
    {
        if (!_previous.TryGetValue(a.Id, out var pa) || !_previous.TryGetValue(b.Id, out var pb))
        {
            return;
        }
        var x = AxisScale(MathF.Abs(pa.X - pb.X), MathF.Abs(a.X - b.X));
        var y = AxisScale(MathF.Abs(pa.Y - pb.Y), MathF.Abs(a.Y - b.Y));
        if (x == 1f && y == 1f)
        {
            return;
        }
        state.Pinch(
            xScale: x,
            yScale: y,
            focusX: ((a.X + b.X) / 2f) - metrics.Gutter,
            focusY: ((a.Y + b.Y) / 2f) - metrics.ContentTop,
            viewport: viewport());
    }

    /// <summary>One axis's scale, or exactly <c>1</c> when the fingers are too close together on it to
    /// know.</summary>
    internal static float AxisScale(float before, float after) =>
        before < MinSpreadPx || after < MinSpreadPx ? 1f : after / before;
}
