// The single gesture owner's state machine: the strip, the zoom, and where a swipe lands.
//
// A port of Android's CalendarSurfaceStateTest, the same fixture, the same numbers. That is
// deliberate: docs/calendar.md is a CROSS-PLATFORM contract, and two clients that agree in prose but
// not in arithmetic have not kept it.
//
// **Windows has moved ahead of Android here, and the divergence is deliberate and logged** (§6, and
// the "Known gaps"): the day axis is one continuous strip with a pinned hour ruler, so the grid may
// rest between two weeks. Android still pages, because it has no wheel, no touchpad hands it a pan it
// cannot tell the end of, which is the whole reason this changed. The tests below that have no Android
// twin are marked as such.
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarSurfaceStateTests
{
    // A 700x600 grid with a 52px hour ruler: the day columns get 648px.
    private static SurfaceViewport Viewport(int lanes = 0) => new(
        Width: 700f,
        Height: 600f,
        Gutter: 52f,
        HeaderHeight: 56f,
        LaneHeight: 24f,
        DividerHeight: 1f,
        Lanes: lanes);

    private static CalendarSurfaceState State(int hours = 12, int days = 7) => new(hours, days);

    [Fact]
    public void At_the_whole_week_zoom_a_week_is_exactly_the_viewport()
    {
        // The seven columns fill the day viewport exactly. It is no longer a special zoom for resting
        // purposes, every zoom rests on a day, but the geometry still has to come out even.
        var s = State(days: 7);
        var m = s.Metrics(Viewport());

        Assert.Equal(648f, m.DayViewport, 0.01f);
        Assert.Equal(648f, m.WeekWidth, 0.01f);
    }

    [Fact]
    public void A_drag_just_moves_the_strip()
    {
        // One axis, one number. There is no day-strip-first-then-the-week hand-off any more: the days
        // are one continuous strip, and a week is a distance along it.
        var s = State(days: 7);
        var m = s.Metrics(Viewport());

        s.PanX(-40f, m);
        Assert.Equal(40f, s.StripX, 0.01f);
        Assert.Equal(0, s.Week);

        // ...and back again, with no unwinding special case: it simply goes the way it came.
        s.PanX(15f, m);
        Assert.Equal(25f, s.StripX, 0.01f);
    }

    [Fact]
    public void Scrolling_off_the_end_of_a_week_runs_into_the_next_one()
    {
        // **No Android twin.** At three columns the week is 1512px and only 648 of it is on screen, so
        // this drag runs off its end, and simply keeps going. It used to stop dead there, hand the
        // remainder to a page turn, and re-seat the days to the new week's FIRST column: a drag from
        // mid-week landed you on a Monday you had not asked for, and the seam was a thing you could
        // feel.
        var s = State(days: 3);
        var m = s.Metrics(Viewport());
        Assert.Equal(216f, m.DayWidth, 0.01f);
        Assert.Equal(1512f, m.WeekWidth, 0.01f);

        s.PanX(-1600f, m);

        // The anchor moved under the strip; the days did not jump.
        Assert.Equal(1, s.Week);
        Assert.Equal(88f, s.StripX, 0.01f); // 1600 - 1512
        Assert.Equal(1600f / 1512f, s.WeekPosition(m), 0.001f);
    }

    [Fact]
    public void A_re_anchor_moves_the_grid_by_exactly_nothing()
    {
        // The one discontinuity the strip has, and the thing that makes it invisible: a week is added to
        // the anchor and its width taken off the offset, and the two cancel in the draw's
        // `(k - Week) * WeekWidth - StripX`. Step across the boundary a pixel at a time and the position
        // Position in weeks, which is what the draw and the animations use, must not so much as stutter.
        var s = State(days: 7);
        var m = s.Metrics(Viewport());
        s.PanX(-647f, m); // one pixel short of the seam
        Assert.Equal(0, s.Week);

        var before = s.WeekPosition(m);
        s.PanX(-2f, m); // ...and two pixels past it
        var after = s.WeekPosition(m);

        Assert.Equal(1, s.Week);
        Assert.Equal(1f, s.StripX, 0.01f);
        Assert.Equal(2f / 648f, after - before, 0.0001f); // it moved exactly the two pixels it was given
    }

    [Fact]
    public void The_strip_runs_backwards_before_the_origin_too()
    {
        // Last week is just a negative distance. Nothing special happens at zero.
        var s = State(days: 7);
        var m = s.Metrics(Viewport());

        s.PanX(100f, m);

        Assert.Equal(-1, s.Week);
        Assert.Equal(548f, s.StripX, 0.01f); // 648 - 100
        Assert.Equal(-100f / 648f, s.WeekPosition(m), 0.001f);
    }

    [Fact]
    public void The_strip_rests_on_the_nearest_day()
    {
        // The whole resting rule, in one number. A day is a seventh of a week, so NearestDay rounds the
        // fractional week position to a seventh, and it is the SAME rule at every zoom, which is why
        // there is no longer a "does this zoom snap?" branch anywhere.
        var s = State(days: 7);
        var m = s.Metrics(Viewport());

        // 100px in, at 92.57px per day: 1.08 days along. The nearest day is the first.
        s.PanX(-100f, m);
        Assert.Equal(1f / 7f, s.NearestDay(m), 0.0001f);

        // Past the halfway mark of the second day, it rounds on rather than back.
        s.PanX(-60f, m); // 160px = 1.73 days
        Assert.Equal(2f / 7f, s.NearestDay(m), 0.0001f);
    }

    [Fact]
    public void A_landing_never_moves_the_grid_by_more_than_half_a_day()
    {
        // This is what makes a landing read as the grid settling rather than as it overruling the user,
        // and it is the whole reason the resting unit is a day and not a week. A week-sized landing can
        // drag the days sideways by three and a half of them; this one cannot move them by more than
        // half of one, from anywhere.
        var s = State(days: 7);
        var m = s.Metrics(Viewport());
        var halfDay = m.DayWidth / 2f;

        for (var px = 0f; px < 648f; px += 7f)
        {
            var fresh = State(days: 7);
            fresh.PanX(-px, m);
            var travel = MathF.Abs((fresh.NearestDay(m) - fresh.WeekPosition(m)) * m.WeekWidth);
            Assert.True(
                travel <= halfDay + 0.01f,
                $"landing from {px}px moved the grid {travel}px, more than half a day ({halfDay}px)");
        }
    }

    [Fact]
    public void The_nearest_day_at_a_sub_week_zoom_is_still_a_day()
    {
        // At three columns a week is 1512px and a day 216px. The rule does not change with the zoom,
        // which is the point: the user asked for narrower columns, not for a different idea of where
        // the grid is allowed to stop.
        var s = State(days: 3);
        var m = s.Metrics(Viewport());
        Assert.Equal(216f, m.DayWidth, 0.01f);

        s.PanX(-500f, m); // 2.31 days along
        Assert.Equal(2f / 7f, s.NearestDay(m), 0.0001f);
    }

    [Fact]
    public void The_nearest_day_crosses_a_week_boundary_without_a_seam()
    {
        // The strip re-anchors as it crosses a week (Rebase), so the arithmetic has to survive the
        // anchor moving underneath it. Landing on the last day of one week and the first of the next
        // must be the same kind of event, or the seam becomes a thing you can feel.
        var s = State(days: 7);
        var m = s.Metrics(Viewport());

        s.PanX(-(648f + 10f), m); // just over a week: 7.1 days
        Assert.Equal(1, s.Week);
        Assert.Equal(1f, s.NearestDay(m), 0.0001f);

        s.PanX(-40f, m); // 7.53 days, now nearest the NEXT day, in the following week
        Assert.Equal(1f + (1f / 7f), s.NearestDay(m), 0.0001f);
    }

    [Fact]
    public void The_hours_scroll_and_stop_at_the_ends_of_the_day()
    {
        var s = State(hours: 12);
        var m = s.Metrics(Viewport());

        // A 12-hour horizon over a 600px surface: a 50px hour, so the day is 1200px tall.
        Assert.Equal(50f, m.HourHeight, 0.01f);
        Assert.Equal(1200f, m.GridHeight, 0.01f);

        s.PanY(-200f, m); // finger up: later in the day
        Assert.Equal(200f, s.ScrollY, 0.01f);

        s.PanY(5000f, m);
        Assert.Equal(0f, s.ScrollY, 0.01f);
        s.PanY(-5000f, m);
        Assert.Equal(m.MaxScrollY, s.ScrollY, 0.01f);
        Assert.True(m.MaxScrollY < m.GridHeight, "which is the day, less what is on screen");
    }

    [Fact]
    public void A_pinch_keeps_the_time_under_your_fingers_under_your_fingers()
    {
        // Scrolled to 09:00 with the fingers halfway down the grid, a pinch must leave whatever they
        // are touching exactly where it is, otherwise the grid slides out from under the hand and
        // appears to zoom about the top of the day.
        var s = State(hours: 12, days: 7);
        var v = Viewport();
        var before = s.Metrics(v);

        s.ScrollTo(9 * before.HourHeight, before); // 09:00 at the top of the grid
        const float Focus = 100f; // 100px in: two hours further down, so 11:00 is under the finger
        Assert.Equal(11f, (s.ScrollY + Focus) / before.HourHeight, 0.01f);

        // Spread vertically: fewer hours on screen, a taller hour.
        s.Pinch(xScale: 1f, yScale: 1.5f, focusX: 0f, focusY: Focus, viewport: v);
        var after = s.Metrics(v);
        Assert.True(after.HourHeight > before.HourHeight, "an hour must have got taller");
        Assert.Equal(11f, (s.ScrollY + Focus) / after.HourHeight, 0.01f);
    }

    [Fact]
    public void A_pinch_keeps_the_day_under_your_fingers_under_your_fingers_across_a_seam()
    {
        // **No Android twin.** The day axis used to clamp its focal correction at the week's first day,
        // so a pinch near the end of a week could not pull the focus day back past it, and the grid
        // crept sideways under the fingers instead. The strip has no such edge: it re-anchors onto the
        // week before, and the day under the fingers stays under the fingers.
        var s = State(hours: 12, days: 3);
        var v = Viewport();
        var before = s.Metrics(v);

        s.StripTo(60f, before); // barely into the week, so a zoom-out MUST reach back past its start
        const float Focus = 500f;
        var dayUnderFinger = (s.StripX + Focus) / before.DayWidth;

        s.Pinch(xScale: 0.6f, yScale: 1f, focusX: Focus, focusY: 0f, viewport: v);
        var after = s.Metrics(v);

        Assert.True(after.DayWidth < before.DayWidth, "the columns must have narrowed");
        Assert.Equal(-1, s.Week); // it reached back into the week before, rather than jamming at zero

        // The same day of the strip is still under the finger, counted from the ANCHOR, which moved.
        var now = ((s.Week - 0) * CalendarUnits.DaysInWeek) + ((s.StripX + Focus) / after.DayWidth);
        Assert.Equal(dayUnderFinger, now, 0.01f);
    }

    [Fact]
    public void A_pinch_does_not_pan_the_grid()
    {
        // The old pinch consumed nothing, it had to, or it cancelled the pager's drag, so the
        // scrollers underneath went on reading the same two fingers and dragged the week around while
        // it zoomed. Nobody is reading them now.
        var s = State(hours: 12, days: 3);
        var v = Viewport();
        s.StripTo(300f, s.Metrics(v));
        s.PanY(-100f, s.Metrics(v));
        var stripX = s.StripX;
        var scrollY = s.ScrollY;

        // A pinch that scales neither axis must move nothing at all, however far its midpoint travelled.
        s.Pinch(xScale: 1f, yScale: 1f, focusX: 400f, focusY: 250f, viewport: v);
        Assert.Equal(stripX, s.StripX, 0.001f);
        Assert.Equal(scrollY, s.ScrollY, 0.001f);
    }

    [Fact]
    public void The_shaper_sleeps_while_the_fingers_are_down()
    {
        // Measured on a real diary: a pinch's draw cost 3.4x a swipe's while drawing HALF as many
        // blocks. A swipe holds the column width still, so every text measurement hits the shaper's
        // cache; a pinch moves it every frame, taking the cache key with it. So the width the text is
        // LAID OUT against stops moving for the length of the gesture, while the rectangle it is
        // clipped to keeps tracking the fingers exactly as before.
        var s = State(days: 7);
        var v = Viewport();
        var before = s.Metrics(v).DayWidth;
        Assert.Equal(0f, s.ShapedDayWidth);

        s.BeginZoom(s.Metrics(v));
        Assert.Equal(before, s.ShapedDayWidth, 0.01f);

        // The columns get much wider under the fingers...
        for (var i = 0; i < 6; i++)
        {
            s.Pinch(xScale: 1.2f, yScale: 1f, focusX: 300f, focusY: 200f, viewport: v);
        }
        Assert.True(s.Metrics(v).DayWidth > before * 1.5f, "the geometry must follow the fingers");
        Assert.Equal(before, s.ShapedDayWidth, 0.01f);

        // Fingers up: the labels may re-shape against the width they actually have.
        s.SettleZoom(v);
        Assert.Equal(0f, s.ShapedDayWidth);
    }

    [Fact]
    public void A_settled_pinch_snaps_to_a_rung()
    {
        // A pinch outwards from three columns lands somewhere fractional; the shape it persists must be
        // one of the four the core knows. It snaps to the settled LEVEL's columns, not to the rounded
        // count: a pinch outwards from the week lands on ~6.4 columns, which rounds to SIX, while the
        // level it maps to is the whole WEEK, of seven.
        var s = State(hours: 12, days: 3);
        var v = Viewport();
        s.ResetDays(7);

        var settled = s.SettleZoom(v);

        Assert.Equal(CalendarMode.Week, settled);
    }

    [Fact]
    public void A_shape_picked_from_the_menu_cannot_leave_the_grid_drawn_off_its_own_screen()
    {
        // The regression, in its new clothes. Pick "3 days" from the menu, scroll deep into the week,
        // then pick "Week": the columns widen, and a day offset nobody re-clamped used to draw the whole
        // week a thousand pixels off to the left, the grid came up BLANK, which looks like a rendering
        // crash and is really a stale offset.
        //
        // A strip cannot have that bug: there is no bound to fall outside of. What it can be left is
        // mid-week by a zoom, so it re-anchors, and the invariant the draw needs (the offset is inside
        // its own week, so at most two weeks are on screen) holds by construction.
        var s = State(days: 3);
        var v = Viewport();
        s.StripTo(1400f, s.Metrics(v)); // deep into a 1512px week
        Assert.Equal(0, s.Week);

        s.ResetDays(7); // the menu re-seeds the zoom. It knows nothing about pixels.
        s.ClampScroll(s.Metrics(v));

        var m = s.Metrics(v);
        Assert.InRange(s.StripX, 0f, m.WeekWidth);
        Assert.Equal(2, s.Week); // 1400px is two of the new 648px weeks in, and the anchor says so
    }

    [Fact]
    public void A_week_with_fewer_all_day_lanes_cannot_leave_the_day_scrolled_past_midnight()
    {
        // Scroll to the bottom of a week whose banner is three lanes tall, then scroll on to a week with
        // none: the banner's rows go back to the grid, the grid gets taller, MaxScrollY shrinks, and a
        // scroll offset nobody re-clamped is now past the end of the day, showing a strip of nothing
        // below midnight.
        var s = State(hours: 12);
        var busy = Viewport(lanes: 3);
        s.PanY(-99_999f, s.Metrics(busy)); // hard against the bottom of the day
        var bottom = s.ScrollY;
        Assert.Equal(s.Metrics(busy).MaxScrollY, bottom, 0.01f);

        var quiet = Viewport(lanes: 0);
        Assert.True(
            s.Metrics(quiet).MaxScrollY < bottom,
            "a week with no banner has more grid, so less to scroll");

        s.ClampScroll(s.Metrics(quiet));
        Assert.Equal(s.Metrics(quiet).MaxScrollY, s.ScrollY, 0.01f);
    }

    [Fact]
    public void Expanding_the_all_day_banner_costs_the_grid_its_room()
    {
        // The banner grows downwards into the grid, so the hours on screen shrink and the scroll has
        // further to go. An hour does NOT change height, the horizon is measured against the whole
        // surface, so a busier week cannot silently rescale the grid.
        var s = State(hours: 12);
        const int Lanes = 5;
        var collapsed = s.Metrics(Viewport(Lanes));
        s.ToggleBanner();
        var expanded = s.Metrics(Viewport(Lanes));

        Assert.Equal(collapsed.HourHeight, expanded.HourHeight, 0.01f);
        Assert.True(expanded.BannerHeight > collapsed.BannerHeight, "the banner must grow");
        Assert.True(
            expanded.ContentHeight < collapsed.ContentHeight,
            "and the grid must give up the room");
        Assert.True(
            expanded.MaxScrollY > collapsed.MaxScrollY,
            "so there is more day to scroll through");
    }

    [Fact]
    public void The_collapsed_banner_never_grows_past_its_cap_however_busy_the_week()
    {
        var s = State();

        // Three lanes fit with no overflow row; twenty do not, and the banner still stops at three.
        Assert.Equal(3, s.Metrics(Viewport(lanes: 3)).BannerLanes);
        Assert.Equal(CalendarAllDay.CollapsedLanes, s.Metrics(Viewport(lanes: 20)).BannerLanes);

        s.ToggleBanner();
        Assert.Equal(20, s.Metrics(Viewport(lanes: 20)).BannerLanes);
    }

    [Fact]
    public void Only_the_anchor_week_and_the_one_after_it_can_ever_be_on_screen()
    {
        // What the pinned hour ruler buys, and what the draw path leans on: the viewport is never wider
        // than a week (a zoom shows FEWER of the seven columns, never more), and the offset is kept
        // inside its own week, so at most one seam is visible, and the banner's height is the larger of
        // exactly two weeks.
        var s = State(days: 7);
        var m = s.Metrics(Viewport());

        Assert.False(s.SecondWeekVisible(m)); // resting on a boundary: one week, its own lanes

        s.PanX(-1f, m);
        Assert.True(s.SecondWeekVisible(m)); // a pixel off it: a seam, and two weeks' lanes to reconcile

        s.PanX(1f, m);
        Assert.False(s.SecondWeekVisible(m));
    }
}
