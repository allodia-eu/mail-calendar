// The header's < and > pressed faster than the grid slides.
//
// A chevron is an animated step of the strip, so a second press can land while the first is still
// sliding. The clock is the test's, as in CalendarFlickTests, so the press arrives one frame into the
// slide rather than after the grid has politely settled.
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

        /// <summary>One chevron press, exactly what <c>CalendarSurface.StepDays</c> does.</summary>
        internal void Press(int days) => Driver.SlideDays(days, Metrics);

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
        // Ten presses, one frame apart, go ten weeks back and rest on the first column. Stepping from
        // where the strip happened to be mid-slide counted each press as a fraction of a week and left
        // the grid resting part of the way into a column.
        var grid = new Grid();
        for (var i = 0; i < 10; i++)
        {
            grid.Press(-CalendarUnits.DaysInWeek);
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
        grid.Press(3);
        grid.Advance(OneFrameMs);
        grid.Press(-3);
        grid.Settle();

        Assert.Equal(3f / CalendarUnits.DaysInWeek, grid.Position, 0.001f);
    }

    [Fact]
    public void A_press_mid_coast_steps_from_the_day_the_coast_is_nearest()
    {
        // A wheel or a flick is still coasting when the chevron is pressed. Its fraction of a column is
        // not the user's to keep: the step goes from the nearest day, as the coast itself would have.
        var grid = new Grid();
        grid.Driver.GlideBy(grid.Metrics.DayWidth * 0.4f, grid.Metrics);
        grid.Advance(OneFrameMs * 3);
        Assert.True(grid.OffDay > 0.01f, "the glide had not moved the strip off its day");

        grid.Press(CalendarUnits.DaysInWeek);
        grid.Settle();

        Assert.Equal(0f, grid.OffDay, 0.001f);
    }

    [Fact]
    public void A_press_at_rest_steps_from_where_the_strip_is_without_realigning_the_week()
    {
        // Wednesday to Tuesday stays Wednesday to Tuesday: a chevron moves by the visible span and
        // never by an amount the user did not ask for.
        var grid = new Grid();
        grid.Press(2);
        grid.Settle();
        grid.Press(CalendarUnits.DaysInWeek);
        grid.Settle();

        Assert.Equal(1f + (2f / CalendarUnits.DaysInWeek), grid.Position, 0.001f);
    }
}
