// Dragging on the grid: what a press meant, and where a drop would leave the event.
//
// The twin of Android's `CalendarDrag.kt` and Apple's `CalendarDragModel.swift`, case for case:
// drag is a cross-platform contract (docs/calendar.md §10), and two clients that agree in prose but
// not in arithmetic have not kept it. Pure, so Mailcal.Tests links it (CalendarDragTests).
//
// **A drag is a delta, not a destination.** What crosses the FFI is how far the hand moved, in whole
// days and minutes, never the clock the block was dropped under (`EventIntent.Move`).
using System;
using System.Collections.Generic;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>What a press on the grid turned out to be.</summary>
internal enum CalendarDragKind
{
    /// <summary>The whole block moves: both edges by one delta, so the duration survives exactly.</summary>
    Move,

    /// <summary>The top edge moves; the end stays.</summary>
    ResizeStart,

    /// <summary>The bottom edge moves; the start stays.</summary>
    ResizeEnd,

    /// <summary>Empty grid: a new slot is drawn out.</summary>
    Create,
}

/// <summary>
/// The event a move or resize reshapes, captured when the press landed.
/// </summary>
/// <remarks>
/// Captured rather than looked up per frame: a sync can re-pull the page mid-gesture, and a drag that
/// re-resolved its subject would jump to another block, or to none. <see cref="OccurrenceStart"/> is
/// the core's token, empty for a one-off, passed back verbatim and never parsed here.
/// </remarks>
internal sealed record CalendarDragSubject(
    string Account,
    string Event,
    string OccurrenceStart,
    int Day,
    int StartMinutes,
    int EndMinutes);

/// <summary>A block the press may land on: where it is drawn, and the event, when it is the user's
/// own and whole (<see cref="CalendarDrag.Movable"/>).</summary>
internal readonly record struct CalendarDragTarget(BlockSpan Span, CalendarDragSubject? Subject);

/// <summary>Where a drag would leave a block, in the core's own geometry.</summary>
internal readonly record struct CalendarDragPreview(int Day, int StartMinutes, int EndMinutes)
{
    /// <summary>How long the block runs, in minutes.</summary>
    internal int Minutes => EndMinutes - StartMinutes;

    /// <summary>The preview as a block the renderer's own <see cref="SurfaceMetrics.BlockRect"/> places.</summary>
    internal BlockSpan Span => new(Day, 0, 1, StartMinutes, EndMinutes);
}

/// <summary>The slot a finished create hands the event editor.</summary>
internal readonly record struct CreateSlot(DateOnly Date, int StartMinutes, int EndMinutes)
{
    /// <summary>The start as the editor's wall clock.</summary>
    internal DateTime Start => Date.ToDateTime(TimeOnly.MinValue).AddMinutes(StartMinutes);

    /// <summary>The end as the editor's wall clock; a slot ending at 24:00 ends at the next midnight.</summary>
    internal DateTime End => Date.ToDateTime(TimeOnly.MinValue).AddMinutes(EndMinutes);
}

/// <summary>The write a settled move or resize asks for (<c>EventIntent.Move</c>).</summary>
/// <remarks><see cref="Occurrence"/> <c>null</c> moves the whole series; a token names one occurrence.</remarks>
internal readonly record struct CalendarMoveArgs(
    string Account, string Key, EventEdge Edge, int Days, int Minutes, string? Occurrence);

/// <summary>
/// A drag in flight: where it began and where the pointer is, in the core's own currency, so the
/// deltas are exactly what <c>EventIntent.Move</c> wants.
/// </summary>
/// <remarks>
/// <see cref="Minute"/> steps a quarter hour at a time because that is what is written;
/// <see cref="RawMinute"/> is where the pointer actually is, what the block is drawn from, so it
/// glides rather than jumping a dozen pixels at a wide horizon. <see cref="RawAnchorMinute"/> is
/// where the press landed, unrounded, which picks a create's hour band.
/// </remarks>
internal sealed record CalendarDragState(
    CalendarDragKind Kind,
    CalendarDragSubject? Subject,
    int AnchorDay,
    int AnchorMinute,
    int Day,
    int Minute,
    int RawMinute,
    int RawAnchorMinute)
{
    internal int DayDelta => Day - AnchorDay;

    internal int MinuteDelta => Minute - AnchorMinute;

    private int RawMinuteDelta => RawMinute - AnchorMinute;

    /// <summary>The pointer moved: re-aimed at where it is now, snapped and unsnapped alike.</summary>
    internal CalendarDragState MovedTo(int day, int minute, int rawMinute) =>
        this with { Day = day, Minute = minute, RawMinute = rawMinute };

    /// <summary>Where a drop would leave things: what the readout says and the write does.</summary>
    internal CalendarDragPreview Preview => PreviewUsing(MinuteDelta);

    /// <summary>Where the block is drawn while the pointer is down: the edge in the hand follows the
    /// pointer between snap steps, the anchored edge sits where the write will put it.</summary>
    internal CalendarDragPreview LivePreview => PreviewUsing(RawMinuteDelta);

    /// <summary>Whether a drop writes anything. A press that went nowhere on an event is a press,
    /// not an edit; a create always does, because the press itself was the request.</summary>
    internal bool MovesAnything => Kind switch
    {
        CalendarDragKind.Create => true,
        CalendarDragKind.Move => DayDelta != 0 || MinuteDelta != 0,
        _ => MinuteDelta != 0,
    };

    /// <summary>Whether a settled drag asks "this event, or all of them?" before it writes.</summary>
    internal bool AsksAboutTheSeries => !string.IsNullOrEmpty(Subject?.OccurrenceStart);

    /// <summary>
    /// The drag clamped so its preview stays inside the grid it is drawn on.
    /// </summary>
    /// <remarks>
    /// What you see is what you get (§10): a move stops at its day's own midnights rather than
    /// landing on the day before, and sideways it stays inside its week. A resize clamps its edge a
    /// quarter hour short of the opposite one rather than refusing.
    /// </remarks>
    internal CalendarDragState Clamped(int columns)
    {
        var (lo, hi) = Kind switch
        {
            CalendarDragKind.Move when Subject is { } s =>
                (AnchorMinute - s.StartMinutes, AnchorMinute + (CalendarDrag.DayMinutes - s.EndMinutes)),
            CalendarDragKind.ResizeStart when Subject is { } s =>
                (AnchorMinute - s.StartMinutes,
                    AnchorMinute + (s.EndMinutes - CalendarDrag.DragSnapMinutes - s.StartMinutes)),
            CalendarDragKind.ResizeEnd when Subject is { } s =>
                (AnchorMinute - (s.EndMinutes - s.StartMinutes - CalendarDrag.DragSnapMinutes),
                    AnchorMinute + (CalendarDrag.DayMinutes - s.EndMinutes)),
            _ => (0, CalendarDrag.DayMinutes),
        };
        (lo, hi) = (Math.Min(lo, hi), Math.Max(lo, hi));
        return this with
        {
            Day = Math.Clamp(Day, 0, Math.Max(columns - 1, 0)),
            Minute = Math.Clamp(Minute, lo, hi),
            RawMinute = Math.Clamp(RawMinute, lo, hi),
        };
    }

    /// <summary>
    /// The write a settled drag asks for, or <c>null</c> for a create.
    /// </summary>
    /// <remarks>
    /// <paramref name="thisOccurrenceOnly"/> is the user's answer to "this event, or all of them?".
    /// A one-off names no occurrence whatever it is passed: its token is empty.
    /// </remarks>
    internal CalendarMoveArgs? MoveArgs(bool thisOccurrenceOnly)
    {
        if (Subject is not { } subject)
        {
            return null;
        }
        var edge = Kind switch
        {
            CalendarDragKind.ResizeStart => EventEdge.Start,
            CalendarDragKind.ResizeEnd => EventEdge.End,
            _ => EventEdge.Whole,
        };
        var occurrence = thisOccurrenceOnly && subject.OccurrenceStart.Length > 0
            ? subject.OccurrenceStart
            : null;
        return new CalendarMoveArgs(subject.Account, subject.Event, edge, DayDelta, MinuteDelta, occurrence);
    }

    private CalendarDragPreview PreviewUsing(int delta)
    {
        switch (Kind)
        {
            case CalendarDragKind.Create:
                // The union of the hour the press is inside and the pointer (§10): continuous, so
                // there is no threshold at which the slot changes shape and nothing can jump at one.
                var band = CalendarDrag.HourBand(RawAnchorMinute);
                var pointer = AnchorMinute + delta;
                return new CalendarDragPreview(
                    AnchorDay, Math.Min(band, pointer), Math.Max(band + CalendarDrag.HourMinutes, pointer));
            case CalendarDragKind.Move when Subject is { } s:
                return new CalendarDragPreview(s.Day + DayDelta, s.StartMinutes + delta, s.EndMinutes + delta);
            case CalendarDragKind.ResizeStart when Subject is { } s:
                return new CalendarDragPreview(s.Day, s.StartMinutes + delta, s.EndMinutes);
            case CalendarDragKind.ResizeEnd when Subject is { } s:
                return new CalendarDragPreview(s.Day, s.StartMinutes, s.EndMinutes + delta);
            default:
                return new CalendarDragPreview(Day, 0, 0);
        }
    }
}

/// <summary>Turning a point into the core's geometry, and a press into a drag.</summary>
internal static class CalendarDrag
{
    /// <summary>The quarter hour the delta snaps to (§10): what a diary is written in. The delta is
    /// what snaps, so an event at 10:07 keeps its seven minutes when moved a day sideways.</summary>
    internal const int DragSnapMinutes = 15;

    /// <summary>How close to a block's edge a press must land to resize rather than move.</summary>
    internal const float ResizeEdge = 8f;

    internal const int HourMinutes = 60;
    internal const int DayMinutes = CalendarUnits.HoursInDay * HourMinutes;

    /// <summary>The hour band a press fills: the one the pointer is <b>inside</b>, from the
    /// unrounded minute, so a press at 17:50 fills 17:00–18:00 rather than the hour below it.</summary>
    internal static int HourBand(int rawMinute) =>
        Math.Clamp(rawMinute / HourMinutes * HourMinutes, 0, DayMinutes - HourMinutes);

    /// <summary>The column a week-space x falls in.</summary>
    internal static int Column(float x, float dayWidth) =>
        dayWidth <= 0f ? 0 : (int)MathF.Floor(x / dayWidth);

    /// <summary>The wall-clock minute a content-space y falls on, to the minute.</summary>
    internal static int RawMinute(float y, float hourHeight) =>
        hourHeight <= 0f || !float.IsFinite(y) ? 0 : (int)MathF.Round(y / hourHeight * HourMinutes);

    /// <summary>The wall-clock minute a content-space y falls on, snapped to the drag grid.</summary>
    internal static int Minute(float y, float hourHeight) =>
        (int)MathF.Round(RawMinute(y, hourHeight) / (float)DragSnapMinutes) * DragSnapMinutes;

    /// <summary>Whether a block is tall enough for two grab zones and a middle between them.</summary>
    internal static bool ResizeZoneApplies(float blockHeight) => blockHeight >= ResizeEdge * 3f;

    /// <summary>The subject a segment offers a drag: the user's own (<c>CanMove</c>, the core's
    /// answer) and whole. A segment clipped by midnight is a clip, not the event, so every gesture on
    /// it would mean something other than what it looks like.</summary>
    internal static CalendarDragSubject? Movable(TimedSegment seg) =>
        seg.CanMove && !seg.ContinuesBefore && !seg.ContinuesAfter
            ? new CalendarDragSubject(
                seg.Account, seg.Event, seg.OccurrenceStart,
                (int)seg.Day, (int)seg.StartMinutes, (int)seg.EndMinutes)
            : null;

    /// <summary>
    /// What a press at (<paramref name="x"/>, <paramref name="y"/>) in one week's content space
    /// means, or <c>null</c>.
    /// </summary>
    /// <remarks>
    /// A block that is the user's own claims the press, its edges as a resize; bare grid claims it as
    /// a create when <paramref name="canCreate"/>. Hit-tested against the rectangle the renderer
    /// draws (<see cref="SurfaceMetrics.BlockRect"/>), so a pointer and the pixels agree.
    /// </remarks>
    internal static CalendarDragState? Press(
        float x,
        float y,
        IReadOnlyList<CalendarDragTarget> targets,
        int dayCount,
        SurfaceMetrics m,
        bool canCreate)
    {
        var day = Column(x, m.DayWidth);
        if (day < 0 || day >= dayCount || y < 0f || y > m.GridHeight)
        {
            return null;
        }
        var minute = Minute(y, m.HourHeight);
        var raw = RawMinute(y, m.HourHeight);

        foreach (var target in targets)
        {
            var r = m.BlockRect(target.Span);
            if (target.Subject is not { } subject || !r.Contains(x, y))
            {
                continue;
            }
            var kind = CalendarDragKind.Move;
            if (ResizeZoneApplies(r.Height))
            {
                if (y - r.Top <= ResizeEdge)
                {
                    kind = CalendarDragKind.ResizeStart;
                }
                else if (r.Bottom - y <= ResizeEdge)
                {
                    kind = CalendarDragKind.ResizeEnd;
                }
            }
            // A resize anchors on the edge it grabbed, not on the pointer: otherwise the first frame
            // jumps the edge to wherever inside the grab zone the pointer landed.
            var anchor = kind switch
            {
                CalendarDragKind.ResizeStart => subject.StartMinutes,
                CalendarDragKind.ResizeEnd => subject.EndMinutes,
                _ => minute,
            };
            return new CalendarDragState(kind, subject, day, anchor, day, anchor, anchor, anchor);
        }

        return canCreate
            ? new CalendarDragState(CalendarDragKind.Create, null, day, minute, day, minute, raw, raw)
            : null;
    }
}
