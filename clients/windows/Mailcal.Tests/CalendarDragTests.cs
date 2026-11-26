// Dragging on the grid: the decisions, without a pointer.
//
// Everything that can be wrong about a drag is arithmetic: what a press meant, where a drop would
// leave the block, and what crosses the FFI. These are the cases, with the same names, of Android's
// `CalendarDragTest` and Apple's `CalendarDragTests`, plus the create cases of Linux's `drag.rs`:
// drag is a cross-platform contract, and two clients that agree in prose but not in arithmetic have
// not kept it. The gesture half is CalendarDragGestureTests.
using System;
using System.Collections.Generic;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarDragTests
{
    // An hour is 60 px tall and a day 100 px wide, so a pixel is a minute.
    private static readonly SurfaceMetrics Grid = new(
        new SurfaceViewport(752f, 600f, 52f, 56f, 24f, 1f, 0), HourHeight: 60f, DayWidth: 100f, BannerLanes: 0);

    private static TimedSegment Segment(
        int day = 1,
        int start = 9 * 60,
        int end = 10 * 60,
        bool canMove = true,
        string occurrence = "",
        bool continuesBefore = false) =>
        new("acct", "evt", "work", "Standup", (uint)day, (uint)start, (uint)end, 0, 1,
            continuesBefore, false, true, canMove, occurrence, ResponseStatus.Accepted);

    private static CalendarDragTarget Target(TimedSegment seg) => new(
        new BlockSpan((int)seg.Day, 0, 1, (int)seg.StartMinutes, (int)seg.EndMinutes),
        CalendarDrag.Movable(seg));

    /// <summary>A week-space point in column <paramref name="day"/> at wall-clock <paramref name="minute"/>.</summary>
    private static CalendarDragState? Press(
        int day, int minute, float inset = 0f, TimedSegment[]? segments = null, bool canCreate = true)
    {
        var targets = new List<CalendarDragTarget>();
        foreach (var seg in segments ?? [Segment()])
        {
            targets.Add(Target(seg));
        }
        return CalendarDrag.Press(
            (100f * day) + 50f, minute + inset, targets, CalendarUnits.DaysInWeek, Grid, canCreate);
    }

    // ---- What a press meant ------------------------------------------------------------------

    [Fact]
    public void A_press_in_the_middle_of_our_own_block_moves_it()
    {
        var drag = Press(1, (9 * 60) + 30);
        Assert.Equal(CalendarDragKind.Move, drag?.Kind);
        Assert.Equal("evt", drag?.Subject?.Event);
    }

    [Fact]
    public void A_press_near_an_edge_resizes_that_edge_and_anchors_on_it()
    {
        // Anchored on the edge, not the pointer, so the first frame does not jump the edge to
        // wherever inside the grab zone the pointer landed.
        var top = Press(1, 9 * 60, inset: 3f);
        Assert.Equal(CalendarDragKind.ResizeStart, top?.Kind);
        Assert.Equal(9 * 60, top?.AnchorMinute);

        var bottom = Press(1, 10 * 60, inset: -3f);
        Assert.Equal(CalendarDragKind.ResizeEnd, bottom?.Kind);
        Assert.Equal(10 * 60, bottom?.AnchorMinute);
    }

    [Fact]
    public void A_block_too_short_for_two_grab_zones_is_always_a_move()
    {
        var shortBlock = new[] { Segment(start: 9 * 60, end: (9 * 60) + 15) };
        Assert.Equal(CalendarDragKind.Move, Press(1, 9 * 60, inset: 2f, segments: shortBlock)?.Kind);
    }

    [Fact]
    public void A_meeting_we_do_not_own_is_not_picked_up()
    {
        // `CanMove` is the core's answer, narrower than a writable calendar. The surface also refuses
        // to draw a slot over any event, so a click on the meeting still opens it.
        var drag = Press(1, (9 * 60) + 30, segments: [Segment(canMove: false)], canCreate: false);
        Assert.Null(drag);
    }

    [Fact]
    public void A_segment_clipped_by_midnight_is_not_picked_up()
    {
        var overnight = new[] { Segment(start: 0, end: 8 * 60, continuesBefore: true) };
        Assert.Null(Press(1, 4 * 60, segments: overnight, canCreate: false));
    }

    [Fact]
    public void A_press_creates_nothing_when_no_calendar_can_be_written() =>
        Assert.Null(Press(3, 14 * 60, segments: [], canCreate: false));

    [Fact]
    public void A_point_past_the_last_column_is_not_a_drag() =>
        Assert.Null(Press(9, 100));

    [Fact]
    public void Minutes_snap_to_the_quarter_hour()
    {
        Assert.Equal(9 * 60, CalendarDrag.Minute((60f * 9) + 4f, 60f));
        Assert.Equal((9 * 60) + 15, CalendarDrag.Minute((60f * 9) + 12f, 60f));
    }

    // ---- Where a drop would leave it ---------------------------------------------------------

    private static readonly CalendarDragSubject Standup = new("acct", "evt", string.Empty, 1, 540, 600);

    private static CalendarDragState Drag(
        CalendarDragKind kind, CalendarDragSubject? subject, int anchorDay, int anchorMinute, int day, int minute) =>
        new(kind, subject, anchorDay, anchorMinute, day, minute, minute, anchorMinute);

    private static CalendarDragPreview Move(int dayDelta, int minuteDelta) =>
        Drag(CalendarDragKind.Move, Standup, 1, 540, 1 + dayDelta, 540 + minuteDelta)
            .Clamped(CalendarUnits.DaysInWeek).Preview;

    [Fact]
    public void A_move_carries_both_edges_by_the_same_amount()
    {
        var preview = Move(1, 30);
        Assert.Equal(new CalendarDragPreview(2, 570, 630), preview);
        Assert.Equal(60, preview.Minutes);
    }

    [Fact]
    public void A_move_is_clamped_inside_its_own_day()
    {
        Assert.Equal(0, Move(0, -900).StartMinutes);
        Assert.Equal(CalendarDrag.DayMinutes, Move(0, 900).EndMinutes);
    }

    [Fact]
    public void A_move_is_clamped_inside_the_week()
    {
        Assert.Equal(0, Move(-5, 0).Day);
        Assert.Equal(CalendarUnits.DaysInWeek - 1, Move(12, 0).Day);
    }

    [Fact]
    public void A_resize_moves_one_edge_and_cannot_pass_the_other()
    {
        var start = Drag(CalendarDragKind.ResizeStart, Standup, 1, 540, 1, 540 + 600)
            .Clamped(CalendarUnits.DaysInWeek).Preview;
        Assert.Equal(600, start.EndMinutes);
        Assert.Equal(600 - 15, start.StartMinutes);

        var end = Drag(CalendarDragKind.ResizeEnd, Standup, 1, 600, 1, 600 - 600)
            .Clamped(CalendarUnits.DaysInWeek).Preview;
        Assert.Equal(540, end.StartMinutes);
        Assert.Equal(540 + 15, end.EndMinutes);
    }

    [Fact]
    public void A_press_that_never_moved_draws_an_hour()
    {
        var create = Drag(CalendarDragKind.Create, null, 2, 600, 2, 600).Preview;
        Assert.Equal(new CalendarDragPreview(2, 600, 660), create);
    }

    [Fact]
    public void A_press_that_was_dragged_draws_what_the_hand_described() =>
        Assert.Equal(690, Drag(CalendarDragKind.Create, null, 2, 600, 2, 690).Preview.EndMinutes);

    [Fact]
    public void A_slot_dragged_upwards_runs_from_the_pointer_to_the_hour_it_began_in() =>
        Assert.Equal(
            new CalendarDragPreview(2, 510, 660),
            Drag(CalendarDragKind.Create, null, 2, 600, 2, 510).Preview);

    [Fact]
    public void A_slot_stays_in_the_column_it_began_in() =>
        Assert.Equal(2, Drag(CalendarDragKind.Create, null, 2, 600, 5, 690).Preview.Day);

    [Fact]
    public void A_press_that_went_nowhere_writes_nothing()
    {
        Assert.False(Drag(CalendarDragKind.Move, Standup, 1, 540, 1, 540).MovesAnything);
        Assert.True(Drag(CalendarDragKind.Create, null, 1, 540, 1, 540).MovesAnything);
    }

    // ---- A create's hour band, the cases of Linux's `drag.rs` ---------------------------------

    private static CalendarDragState Create(int rawMinute) =>
        Press(2, rawMinute, segments: [])!;

    private static CalendarDragState Moved(CalendarDragState drag, int rawMinute) =>
        drag.MovedTo(drag.Day, CalendarDrag.Minute(rawMinute, 60f), rawMinute)
            .Clamped(CalendarUnits.DaysInWeek);

    [Fact]
    public void A_press_fills_the_hour_band_the_pointer_is_inside() =>
        Assert.Equal(new CalendarDragPreview(2, 16 * 60, 17 * 60), Create((16 * 60) + 53).Preview);

    [Fact]
    public void Dragging_down_and_up_takes_the_union_with_that_hour()
    {
        Assert.Equal(
            new CalendarDragPreview(2, 10 * 60, (11 * 60) + 30),
            Moved(Create((10 * 60) + 5), (11 * 60) + 28).Preview);
        Assert.Equal(
            new CalendarDragPreview(2, (8 * 60) + 30, 11 * 60),
            Moved(Create((10 * 60) + 5), (8 * 60) + 28).Preview);
    }

    [Fact]
    public void The_block_glides_while_the_readout_stays_on_the_snap()
    {
        var create = Moved(Create(10 * 60), (11 * 60) + 28);
        Assert.Equal((11 * 60) + 30, create.Preview.EndMinutes);
        Assert.Equal(90, create.Preview.Minutes);
        Assert.Equal((11 * 60) + 28, create.LivePreview.EndMinutes);
    }

    [Fact]
    public void A_create_stays_inside_its_day()
    {
        Assert.Equal(0, Moved(Create(30), -45).Preview.StartMinutes);
        Assert.Equal(24 * 60, Moved(Create((23 * 60) + 30), (24 * 60) + 90).Preview.EndMinutes);
    }

    // ---- What crosses the FFI ------------------------------------------------------------------

    private static readonly CalendarDragSubject Weekly = Standup with { OccurrenceStart = "2026-07-07T09:00:00" };

    [Fact]
    public void A_move_sends_a_delta_not_a_destination()
    {
        var args = Drag(CalendarDragKind.Move, Standup, 1, 540, 2, 570).MoveArgs(thisOccurrenceOnly: false);
        Assert.Equal(EventEdge.Whole, args?.Edge);
        Assert.Equal(1, args?.Days);
        Assert.Equal(30, args?.Minutes);
    }

    [Fact]
    public void Each_resize_names_its_own_edge()
    {
        Assert.Equal(
            EventEdge.Start,
            Drag(CalendarDragKind.ResizeStart, Standup, 1, 540, 1, 525).MoveArgs(false)?.Edge);
        Assert.Equal(
            EventEdge.End,
            Drag(CalendarDragKind.ResizeEnd, Standup, 1, 600, 1, 630).MoveArgs(false)?.Edge);
    }

    [Fact]
    public void A_repeating_event_is_asked_about_and_a_one_off_is_not()
    {
        Assert.True(Drag(CalendarDragKind.Move, Weekly, 1, 540, 2, 540).AsksAboutTheSeries);
        Assert.False(Drag(CalendarDragKind.Move, Standup, 1, 540, 2, 540).AsksAboutTheSeries);
    }

    [Fact]
    public void This_event_names_the_occurrence_and_all_events_names_none()
    {
        var drag = Drag(CalendarDragKind.Move, Weekly, 1, 540, 2, 540);
        Assert.Equal("2026-07-07T09:00:00", drag.MoveArgs(thisOccurrenceOnly: true)?.Occurrence);
        Assert.Null(drag.MoveArgs(thisOccurrenceOnly: false)?.Occurrence);
    }

    [Fact]
    public void A_one_off_never_names_an_occurrence_whatever_it_is_asked() =>
        Assert.Null(Drag(CalendarDragKind.Move, Standup, 1, 540, 2, 540).MoveArgs(true)?.Occurrence);

    [Fact]
    public void A_create_asks_for_no_move_at_all() =>
        Assert.Null(Drag(CalendarDragKind.Create, null, 1, 540, 1, 600).MoveArgs(false));

    // ---- What the editor opens on --------------------------------------------------------------

    [Fact]
    public void A_dragged_slot_opens_the_editor_on_the_time_that_was_drawn()
    {
        var slot = new CreateSlot(new DateOnly(2026, 7, 22), 9 * 60, (10 * 60) + 45);
        Assert.Equal(new DateTime(2026, 7, 22, 9, 0, 0), slot.Start);
        Assert.Equal(new DateTime(2026, 7, 22, 10, 45, 0), slot.End);
        Assert.Equal(DateTimeKind.Unspecified, slot.Start.Kind);

        // Midnight at the end is the next day's 00:00, not the start of this one.
        Assert.Equal(
            new DateTime(2026, 7, 23, 0, 0, 0),
            new CreateSlot(new DateOnly(2026, 7, 22), 23 * 60, 24 * 60).End);
    }
}
