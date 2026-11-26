// Dragging on the grid, the surface's half (docs/calendar.md §10): a new slot drawn out on empty
// space, and the user's own event moved or resized.
//
// The owner decides when a press takes hold (CalendarGestureOwner.Drag.cs) and the arithmetic is
// pure (CalendarDrag.cs). What is left here is the grid's own geometry, inverted: which week and
// column a press is in and what it landed on, then the block drawn where the drop would leave it with
// the time beside it, and the finished drag handed to the host, which opens the editor on a new slot
// or asks the core to move the event.
//
// Split from CalendarSurface.cs to stay under the 500-line file cap; it is the same class.
using System;
using System.Collections.Generic;
using System.Globalization;
using Microsoft.Graphics.Canvas;
using Microsoft.UI.Input;
using Windows.Foundation;

namespace Allodia.Mailcal.Calendar;

internal sealed partial class CalendarSurface : ICalendarDrag
{
    /// <summary>The room around the readout's text, and its distance from the block.</summary>
    private const float PillPadX = 8f;
    private const float PillPadY = 4f;
    private const float PillGap = 6f;

    private CalendarDragState? _drag;
    private int _dragWeek;
    private DateOnly _dragDate;
    private EventColors _dragColors;

    /// <summary>The block a move or resize took hold of, drawn again where the drop would leave it.</summary>
    private BlockPaint? _dragBlock;

    private InputSystemCursorShape _cursor = InputSystemCursorShape.Arrow;

    /// <summary>
    /// The colour of the calendar a new event would go on, or <c>null</c> when there is none, which
    /// is when no press draws a slot, exactly as the "New event" button is then disabled.
    /// </summary>
    internal Func<string?>? NewEventHex { get; set; }

    /// <summary>Raised when a slot has been drawn, to open the event editor on it.</summary>
    internal Action<CreateSlot>? CreateAt { get; set; }

    /// <summary>Raised when an event has been dragged somewhere new, to write it: the host asks
    /// "this event, or all of them?" first when <see cref="CalendarDragState.AsksAboutTheSeries"/>.</summary>
    internal Action<CalendarDragState>? MoveDropped { get; set; }

    bool ICalendarDrag.Begin(float x, float y)
    {
        if (PressAt(x, y, canCreate: true) is not var (week, drag, block))
        {
            return false;
        }
        _drag = drag;
        _dragWeek = week;
        _dragDate = Page(week).Headings[drag.AnchorDay].Date;
        _dragBlock = block;
        _dragColors = block?.Colors
            ?? CalendarColors.EventColorsOf(NewEventHex?.Invoke() ?? CalendarColors.FallbackHex, _theme.Dark);
        Invalidate();
        return true;
    }

    void ICalendarDrag.Move(float x, float y)
    {
        if (_drag is not { } drag)
        {
            return;
        }
        var m = _state.Metrics(Viewport());
        var cx = x - m.Gutter - LeftOf(_dragWeek, m);
        var cy = y - m.ContentTop + _state.ScrollY;
        _drag = drag.MovedTo(
                CalendarDrag.Column(cx, m.DayWidth),
                CalendarDrag.Minute(cy, m.HourHeight),
                CalendarDrag.RawMinute(cy, m.HourHeight))
            .Clamped(Page(_dragWeek).Headings.Count);
        Invalidate();
    }

    bool ICalendarDrag.Finish()
    {
        if (_drag is not { } drag)
        {
            return false;
        }
        EndDrag();
        if (!drag.MovesAnything)
        {
            return false;
        }
        if (drag.Kind == CalendarDragKind.Create)
        {
            var slot = drag.Preview;
            CreateAt?.Invoke(new CreateSlot(_dragDate, slot.StartMinutes, slot.EndMinutes));
        }
        else
        {
            MoveDropped?.Invoke(drag);
        }
        return true;
    }

    void ICalendarDrag.Cancel() => EndDrag();

    private void EndDrag()
    {
        _drag = null;
        _dragBlock = null;
        Invalidate();
    }

    /// <summary>
    /// What a press at (<paramref name="x"/>, <paramref name="y"/>) would take hold of: the week it is
    /// in, the drag, and the block, when it landed on one.
    /// </summary>
    /// <remarks>
    /// A new slot is never drawn over an event, the user's own or anybody else's: a click on a
    /// meeting has to open it. Nor on a week the core has not expanded yet, which is not known to be
    /// empty (§4).
    /// </remarks>
    private (int Week, CalendarDragState Drag, BlockPaint? Block)? PressAt(float x, float y, bool canCreate)
    {
        var m = _state.Metrics(Viewport());
        if (m.DayWidth <= 0f || x < m.Gutter || y < m.ContentTop)
        {
            return null;
        }
        canCreate = canCreate && NewEventHex?.Invoke() is not null && EventAt(x, y) is null;
        var last = _state.SecondWeekVisible(m) ? _state.Week + 1 : _state.Week;
        for (var week = _state.Week; week <= last; week++)
        {
            var page = Page(week);
            var cx = x - m.Gutter - LeftOf(week, m);
            var column = CalendarDrag.Column(cx, m.DayWidth);
            if (column < 0 || column >= page.Headings.Count)
            {
                continue;
            }
            var targets = new List<CalendarDragTarget>(page.Blocks.Count);
            foreach (var block in page.Blocks)
            {
                targets.Add(new CalendarDragTarget(block.Span, block.Movable));
            }
            var drag = CalendarDrag.Press(
                cx, y - m.ContentTop + _state.ScrollY, targets, page.Headings.Count, m,
                canCreate && page.IsMaterialized);
            if (drag is null)
            {
                return null;
            }
            var held = drag.Subject is null
                ? null
                : page.Blocks[targets.FindIndex(t => ReferenceEquals(t.Subject, drag.Subject))];
            return (week, drag, held);
        }
        return null;
    }

    /// <summary>
    /// The mouse pointer over the grid: a resize arrow over an edge that can be dragged, a move
    /// arrow over a block that can, the ordinary arrow elsewhere, so a hand can see what a press
    /// would do before it presses.
    /// </summary>
    private void Hover(float x, float y)
    {
        var shape = PressAt(x, y, canCreate: false)?.Drag.Kind switch
        {
            CalendarDragKind.ResizeStart or CalendarDragKind.ResizeEnd => InputSystemCursorShape.SizeNorthSouth,
            CalendarDragKind.Move => InputSystemCursorShape.SizeAll,
            _ => InputSystemCursorShape.Arrow,
        };
        if (shape != _cursor)
        {
            _cursor = shape;
            ProtectedCursor = InputSystemCursor.Create(shape);
        }
    }

    /// <summary>
    /// The block being dragged, where the drop would leave it, and the time it would land on beside it.
    /// </summary>
    /// <remarks>
    /// The block follows the pointer between snap steps; the readout quotes the snapped times, so
    /// nothing on screen claims a minute the drop will not honour (§10).
    /// </remarks>
    private void DrawDrag(CanvasDrawingSession ds, SurfaceMetrics m)
    {
        if (_drag is not { } drag || _text is null)
        {
            return;
        }
        var live = drag.LivePreview;
        var r = m.BlockRect(live.Span)
            .Translate(m.Gutter + LeftOf(_dragWeek, m), m.ContentTop - _state.ScrollY);
        var rect = new Rect(
            r.Left + SurfaceTheme.BlockGap,
            r.Top + SurfaceTheme.BlockGap,
            MathF.Max(r.Width - (SurfaceTheme.BlockGap * 2f), 0f),
            MathF.Max(r.Height - (SurfaceTheme.BlockGap * 2f), 0f));

        var settled = drag.Preview;
        var culture = CultureInfo.CurrentCulture;
        // A slot ending at midnight ends at the next day's 00:00, which is where the write puts it.
        var start = CalendarFormat.ClockTime(settled.StartMinutes, Use24Hour, culture);
        var end = CalendarFormat.ClockTime(settled.EndMinutes % CalendarDrag.DayMinutes, Use24Hour, culture);
        var label = start + "–" + end;

        using (ds.CreateLayer(1f, new Rect(m.Gutter, m.ContentTop, m.DayViewport, m.ContentHeight)))
        {
            CalendarBlockDraw.Shape(ds, rect, _dragColors, _dragBlock?.Awaiting ?? false);
            if (_dragBlock is { } block)
            {
                var moved = block with { Span = live.Span, Time = start, Range = label };
                var room = m.DayWidth - (SurfaceTheme.BlockGap * 2f) - CalendarBlocks.BarWidth
                    - (CalendarBlocks.LabelPadding * 2f);
                CalendarBlockDraw.BlockLabel(ds, _text, _theme, moved, rect, m.HourHeight, room);
            }
        }
        DrawReadout(ds, m, r, label);
    }

    /// <summary>The time a drag would land on, in a pill beside the block rather than inside it: a
    /// quarter-hour block at a wide horizon is a few pixels tall. Kept on the visible grid, so a block
    /// against an edge cannot push its own readout off it.</summary>
    private void DrawReadout(CanvasDrawingSession ds, SurfaceMetrics m, GridRect block, string label)
    {
        var text = _text!.Run(label, _theme.Chrome);
        var width = text.Width + (PillPadX * 2f);
        var height = text.Height + (PillPadY * 2f);
        if (m.DayViewport < width || m.ContentHeight < height)
        {
            return;
        }
        var x = Math.Clamp(block.Left + ((block.Width - width) / 2f), m.Gutter, m.Width - width);
        var above = block.Top - height - PillGap;
        var y = Math.Clamp(
            above >= m.ContentTop ? above : block.Bottom + PillGap, m.ContentTop, m.Height - height);
        ds.FillRoundedRectangle(x, y, width, height, height / 2f, height / 2f, _theme.Text);
        ds.DrawTextLayout(text.Layout, x + PillPadX, y + PillPadY, _theme.Surface);
    }
}
