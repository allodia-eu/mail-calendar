// The header's < and > pressed faster than the grid slides.
//
// A chevron slides the strip to the first day of the period the core seats the focus in, so a second
// press can land while the first is still sliding. The clock is the test's, as in CalendarFlickTests,
// so the press arrives one frame into the slide rather than after the grid has politely settled.
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarChevronTests
{
    private const float OneFrameMs = 16f;

    private static readonly SurfaceViewport Screen = new(
        Width: 700f,
        Height: 600f,
        Gutter: 52f,
        HeaderHeight: 56f,
        LaneHeight: 24f,
        DividerHeight: 1f,
        Lanes: 0);

    private sealed class Grid
    {
        internal Grid(int days = CalendarUnits.DaysInWeek)
        {
            State = new CalendarSurfaceState(visibleHours: 12, visibleDays: days);
            Driver = new CalendarSurfaceDriver(State);
        }

        internal CalendarSurfaceState State { get; }

        internal CalendarSurfaceDriver Driver { get; }

        internal SurfaceMetrics Metrics => State.Metrics(Screen);

        internal float Position => State.WeekPosition(Metrics);

        /// <summary>One chevron press, exactly what <c>CalendarSurface.SlideTo</c> does: the seat's
        /// first day, <paramref name="days"/> from the origin's.</summary>
        internal void Press(int days) => Driver.SlideTo((float)days / CalendarUnits.DaysInWeek, Metrics);

        internal void Advance(float ms) => Driver.Tick(ms / 1000f, Metrics);

        internal void Settle()
        {
            for (var i = 0; i < 400 && Driver.IsAnimating; i++)
            {
                Advance(OneFrameMs);
            }
        }

        internal float OffDay
        {
            get
            {
                var days = Position * CalendarUnits.DaysInWeek;
                return MathF.Abs(days - MathF.Round(days));
            }
        }
    }

    [Fact]
    public void Every_press_counts_when_the_next_lands_mid_slide()
    {
        // Ten presses, one frame apart, go ten weeks back and rest on the first column: each press
        // names where the focus now is, so one landing mid-slide still counts in full.
        var grid = new Grid();
        for (var i = 1; i <= 10; i++)
        {
            grid.Press(-i * CalendarUnits.DaysInWeek);
            grid.Advance(OneFrameMs);
        }
        grid.Settle();

        Assert.Equal(-10f, grid.Position, 0.001f);
        Assert.Equal(0f, grid.OffDay, 0.001f);
    }

    [Fact]
    public void A_press_mid_slide_the_other_way_undoes_the_one_before()
    {
        var grid = new Grid(days: 3);
        grid.Press(3);
        grid.Advance(OneFrameMs);
        grid.Press(6);
        grid.Advance(OneFrameMs);
        grid.Press(3);
        grid.Settle();

        Assert.Equal(3f / CalendarUnits.DaysInWeek, grid.Position, 0.001f);
    }

    [Fact]
    public void A_press_mid_coast_lands_on_its_seat_not_on_the_coast()
    {
        // A wheel or a flick is still coasting when the chevron is pressed. The press overrules it and
        // lands on a day.
        var grid = new Grid();
        grid.Driver.GlideBy(grid.Metrics.DayWidth * 0.4f, grid.Metrics);
        grid.Advance(OneFrameMs * 3);
        Assert.True(grid.OffDay > 0.01f, "the glide had not moved the strip off its day");

        grid.Press(CalendarUnits.DaysInWeek);
        grid.Settle();

        Assert.Equal(1f, grid.Position, 0.001f);
        Assert.Equal(0f, grid.OffDay, 0.001f);
    }

    [Fact]
    public void A_press_from_a_week_scrolled_off_its_first_day_lands_on_the_next_week_start()
    {
        // Wednesday to Tuesday, then next: the week drawn around the focus, from its first day, so the
        // week view always shows a whole week after a press (docs/calendar.md, "The focus date").
        var grid = new Grid();
        grid.Press(2);
        grid.Settle();
        grid.Press(CalendarUnits.DaysInWeek);
        grid.Settle();

        Assert.Equal(1f, grid.Position, 0.001f);
    }
}
