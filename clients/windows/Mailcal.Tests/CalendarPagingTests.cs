// Week alignment is a deliberate act, not a side-effect (docs/calendar.md §3).
//
// The bug this pins: a 7-column week page opened on *today* rather than on the week's first day, so
// on a Tuesday the grid showed Tuesday under the first heading. The whole scroll-to-today model
// (TodayColumn) already assumes an aligned origin, the origin simply was not being aligned.
//
// The rule has two halves, and both are here:
//   - a *seat* (construct, jump home, switch view) aligns the origin to the core's week start;
//   - a *zoom* never does, widening three columns to seven keeps the same first day, because a
//     Monday-aligned week cannot contain an arbitrary three-day window without the grid jumping (§2).
//
// The aligner is injected, exactly as the real client injects the core's `week_start_date`, so this
// is a plain unit test with no WinUI and no FFI.
using System;
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarPagingTests
{
    // The two week-start conventions, as pure functions of a date, standing in for the core's
    // `week_start_date`, which is all the pager ever knows about alignment.
    private static DateOnly Monday(DateOnly d) => d.AddDays(-(((int)d.DayOfWeek + 6) % 7));

    private static DateOnly Sunday(DateOnly d) => d.AddDays(-(int)d.DayOfWeek);

    // A fixed Tuesday, the day the bug was reported on, and the Monday and Sunday of its week.
    private static readonly DateOnly Tue = new(2026, 7, 14);
    private static readonly DateOnly MonOfWeek = new(2026, 7, 13);
    private static readonly DateOnly SunOfWeek = new(2026, 7, 12);

    [Fact]
    public void A_week_seated_on_a_Tuesday_opens_on_the_Monday()
    {
        // The reported bug, as an assertion: the first column of a week view is the week's first day,
        // not today.
        var pager = new CalendarPager(Tue, CalendarMode.Week, Monday);

        Assert.Equal(MonOfWeek, pager.AnchorFor(0));
        Assert.Equal(DayOfWeek.Monday, pager.AnchorFor(0).DayOfWeek);
        // Today is then its own column within that week, which is where the grid scrolls (§3).
        Assert.Equal(1, CalendarPager.TodayColumn(Tue, pager.AnchorFor(0)));
    }

    [Fact]
    public void A_Sunday_start_week_seats_on_the_Sunday()
    {
        // The setting the core owns, honoured: flip week-start to Sunday and the same Tuesday opens
        // on the Sunday before it.
        var pager = new CalendarPager(Tue, CalendarMode.Week, Sunday);

        Assert.Equal(SunOfWeek, pager.AnchorFor(0));
        Assert.Equal(DayOfWeek.Sunday, pager.AnchorFor(0).DayOfWeek);
        Assert.Equal(2, CalendarPager.TodayColumn(Tue, pager.AnchorFor(0)));
    }

    [Fact]
    public void A_zoom_never_re_aligns_the_week()
    {
        // §2/§3: the days never move on a zoom, only their width. So narrowing to a day and widening
        // back to a week keeps the very same first day. Aligning on a zoom is the jump §2 forbids.
        var pager = new CalendarPager(Tue, CalendarMode.Week, Monday);
        Assert.Equal(MonOfWeek, pager.AnchorFor(0));

        pager.SetZoom(CalendarMode.Day);
        Assert.Equal(MonOfWeek, pager.AnchorFor(0));

        pager.SetZoom(CalendarMode.ThreeDay);
        Assert.Equal(MonOfWeek, pager.AnchorFor(0));

        pager.SetZoom(CalendarMode.Week);
        Assert.Equal(MonOfWeek, pager.AnchorFor(0));
    }

    [Fact]
    public void Paging_strides_whole_weeks_from_the_aligned_origin()
    {
        // A swipe is a different page over the same origin, so each page is exactly seven days on from
        // the aligned first day, never drifting off the week boundary.
        var pager = new CalendarPager(Tue, CalendarMode.Week, Monday);

        Assert.Equal(MonOfWeek.AddDays(7), pager.AnchorFor(1));
        Assert.Equal(MonOfWeek.AddDays(-7), pager.AnchorFor(-1));
        Assert.Equal(DayOfWeek.Monday, pager.AnchorFor(3).DayOfWeek);
    }

    [Fact]
    public void Jumping_home_re_aligns()
    {
        // "Back to today" is a seat, so it aligns: jumping to a Thursday opens on that week's Monday.
        var pager = new CalendarPager(Tue, CalendarMode.Week, Monday);
        pager.SetZoom(CalendarMode.Day);

        pager.JumpTo(new DateOnly(2026, 7, 16)); // a Thursday

        Assert.Equal(MonOfWeek, pager.AnchorFor(0));
        Assert.Equal(DayOfWeek.Monday, pager.AnchorFor(0).DayOfWeek);
    }

    [Fact]
    public void Switching_from_month_to_a_grid_opens_on_the_dates_week()
    {
        // The month anchors on the 1st (a Wednesday in July 2026), so switching to a grid must open on
        // that date's *week*, not mid-week on the 1st.
        var pager = new CalendarPager(Tue, CalendarMode.Month, Monday);
        Assert.Equal(new DateOnly(2026, 7, 1), pager.AnchorFor(0)); // the month anchors on the 1st

        pager.SetMode(CalendarMode.Week, 0);

        Assert.Equal(DayOfWeek.Monday, pager.AnchorFor(0).DayOfWeek);
        Assert.Equal(new DateOnly(2026, 6, 29), pager.AnchorFor(0)); // the Monday of the week of the 1st
    }

    [Fact]
    public void A_grid_view_switch_keeps_the_week_and_stays_aligned()
    {
        // Switching zoom shape from the menu, after swiping two pages on: the origin follows the page
        // the user is looking at, and it is still a Monday (grid-to-grid alignment is idempotent).
        var pager = new CalendarPager(Tue, CalendarMode.Week, Monday);

        pager.SetMode(CalendarMode.Day, 2);

        Assert.Equal(MonOfWeek.AddDays(14), pager.AnchorFor(0));
        Assert.Equal(DayOfWeek.Monday, pager.AnchorFor(0).DayOfWeek);
    }

    [Fact]
    public void The_wide_shapes_frame_from_the_week_start_and_the_narrow_ones_on_today()
    {
        // The product decision (kept identical on Android): the work week opens on Monday and shows
        // Mon–Fri whatever day it is; the narrow shapes frame on today.
        Assert.Equal(0, CalendarPager.FramingColumn(CalendarMode.WorkWeek, Tue, MonOfWeek));

        // Day shows today; 3-day shows today plus the next two (today at the left edge → Tue–Thu).
        Assert.Equal(1, CalendarPager.FramingColumn(CalendarMode.Day, Tue, MonOfWeek));
        Assert.Equal(1, CalendarPager.FramingColumn(CalendarMode.ThreeDay, Tue, MonOfWeek));

        // **The whole week frames on the week's FIRST DAY, and this is now said out loud.** It used to
        // return today's column (1, here) and rely on the surface clamping it away: the day axis could
        // not scroll inside a whole-week zoom, so a non-zero framing column was quietly clipped to zero.
        // The strip has no such bound any more, it runs on through the weeks, so a framing column of 1
        // would open the grid on a week that BEGINS on Tuesday, which is precisely the misalignment §3
        // exists to prevent. An accident of the geometry is now a rule.
        Assert.Equal(0, CalendarPager.FramingColumn(CalendarMode.Week, Tue, MonOfWeek));

        // Work week ignores today entirely, even on a weekend it still opens Mon–Fri.
        var saturday = new DateOnly(2026, 7, 18);
        Assert.Equal(0, CalendarPager.FramingColumn(CalendarMode.WorkWeek, saturday, MonOfWeek));

        // And the 3-day zoom on a Sunday genuinely means Sunday–Tuesday now, running across the week
        // boundary: column 6, with the strip carrying the last two days into the week that follows. It
        // used to clamp back to Friday–Sunday, because the days could not leave their week.
        var sunday = new DateOnly(2026, 7, 19);
        Assert.Equal(6, CalendarPager.FramingColumn(CalendarMode.ThreeDay, sunday, MonOfWeek));
    }

    [Fact]
    public void Without_an_aligner_the_origin_is_the_raw_seed()
    {
        // Back-compat: the aligner is optional, and unset it is the identity, so a test that does not
        // care about alignment (and the pre-Model field initializer) gets exactly the date it passed.
        var pager = new CalendarPager(Tue, CalendarMode.Week);

        Assert.Equal(Tue, pager.AnchorFor(0));
    }
}
