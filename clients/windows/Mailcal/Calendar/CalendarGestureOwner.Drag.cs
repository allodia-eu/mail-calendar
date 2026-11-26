// Which press takes hold of the grid, decided by the same single owner as the pan and the pinch.
//
// docs/calendar.md §10: the gesture is per-platform because the input is. A mouse is a desktop's
// pointer, and a desktop scrolls a calendar with the wheel, so a click-drag draws a new slot, moves
// an event or resizes it. A finger or a pen has no wheel: a plain drag is the only way it can pan, so
// it stays a pan, and a press held still is what takes hold, as on a phone or a tablet.
//
// The hold is a timeout in the same pointer stream as the swipe, the pan and the pinch, which is the
// four-handlers-one-finger arrangement of §6 if it is given a recogniser of its own. So it is one
// more mode of this owner, timed by the frame clock the surface already runs while a contact is held:
// a finger perfectly still raises no pointer events to time it by.
//
// Split from CalendarGestureOwner.cs to stay under the 500-line file cap; it is the same class.
using System;

namespace Allodia.Mailcal.Calendar;

/// <summary>What the surface does with a drag. The owner decides only <i>when</i> one begins.</summary>
internal interface ICalendarDrag
{
    /// <summary>A press at (<paramref name="x"/>, <paramref name="y"/>) asks to take hold there:
    /// the user's own event, its edge, or empty grid. False where nothing may be dragged.</summary>
    bool Begin(float x, float y);

    /// <summary>The pointer holding the drag moved.</summary>
    void Move(float x, float y);

    /// <summary>The pointer lifted. False when the drag changed nothing, which makes it a click.</summary>
    bool Finish();

    /// <summary>The gesture was taken away: nothing is written.</summary>
    void Cancel();
}

internal sealed partial class CalendarGestureOwner
{
    /// <summary>How long a finger or a pen must rest before the press takes hold.</summary>
    /// <remarks>iOS's long press, so a hold means the same on every touch screen this app runs on.</remarks>
    internal const float HoldSeconds = 0.4f;

    /// <summary>How far a mouse moves before a press is a drag. A mouse does not jitter as a finger
    /// does, so it is far less than the touch slop.</summary>
    private const float MouseSlopPx = 3f;

    private bool _mouse;
    private float _held;
    private bool _holdSpent;
    private bool _dragTried;
    private uint _dragId;

    /// <summary>Where a drag goes. Absent, no press takes hold.</summary>
    internal ICalendarDrag? Drag { get; set; }

    /// <summary>Whether a finger is resting and may yet become a hold.</summary>
    private bool HoldPending =>
        _mode == GestureMode.Undecided && !_mouse && !_holdSpent && _pointers.Count == 1;

    /// <summary>The first contact of a gesture went down.</summary>
    private void BeginPress(PointerSample p)
    {
        _mouse = p.Mouse;
        _held = 0f;
        _holdSpent = Drag is null;
        _dragTried = false;
        _dragId = p.Id;
    }

    /// <summary>
    /// A mouse has moved: past its slop it takes hold if the press may, and past the touch slop it is
    /// a drag that does nothing.
    /// </summary>
    /// <remarks>
    /// The second threshold is what keeps a click on somebody else's meeting a click: a hand moves a
    /// mouse a pixel or two while clicking, and a press that may not be dragged stays a tap until it
    /// has travelled as far as a finger's would have to.
    /// </remarks>
    private void DecideMouse(PointerSample p)
    {
        var travel = MathF.Max(MathF.Abs(_travelX), MathF.Abs(_travelY));
        if (!_dragTried && travel > MouseSlopPx)
        {
            _dragTried = true;
            if (Drag?.Begin(_downX, _downY) == true)
            {
                _mode = GestureMode.Drag;
                _dragId = p.Id;
                return;
            }
        }
        if (travel > touchSlop)
        {
            _mode = GestureMode.Inert;
        }
    }

    /// <summary>
    /// One frame of a resting finger: once it has rested for <see cref="HoldSeconds"/>, it takes hold
    /// where it is.
    /// </summary>
    private void TickHold(float dt)
    {
        if (!HoldPending)
        {
            return;
        }
        _held += dt;
        if (_held < HoldSeconds)
        {
            return;
        }
        _holdSpent = true;
        foreach (var p in _pointers.Values)
        {
            if (Drag?.Begin(p.X, p.Y) == true)
            {
                _mode = GestureMode.Drag;
                _dragId = p.Id;
            }
        }
    }
}
