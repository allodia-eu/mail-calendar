// When the time grid shows its loading pill, and where (docs/calendar.md, "Showing a period that is
// loading"): over the grid it leaves drawn, whenever a week on screen is loading, at the top of the
// hours rather than of the day.
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarLoadingTests
{
    private static readonly SurfaceViewport Screen = new(
        Width: 700f,
        Height: 600f,
        Gutter: 52f,
        HeaderHeight: 56f,
        LaneHeight: 24f,
        DividerHeight: 1f,
        Lanes: 2);

    private static SurfaceMetrics Metrics =>
        new CalendarSurfaceState(visibleHours: 12, visibleDays: CalendarUnits.DaysInWeek).Metrics(Screen);

    [Fact]
    public void AWeekOnScreenThatIsLoadingShowsThePill()
    {
        var place = CalendarLoading.Place(0, 6, week => week == 0, Metrics);

        Assert.True(place.Shown);
    }

    [Fact]
    public void AMaterializedWeekShowsNoPill()
    {
        var place = CalendarLoading.Place(0, 6, _ => false, Metrics);

        Assert.False(place.Shown);
    }

    [Fact]
    public void AtASeamTheLoadingNeighbourOnScreenShowsThePill()
    {
        // Thursday of the anchor week to Wednesday of the next: the next week is loading.
        var place = CalendarLoading.Place(3, 9, week => week == 1, Metrics);

        Assert.True(place.Shown);
    }

    [Fact]
    public void ALoadingWeekOffScreenShowsNoPill()
    {
        // The strip holds the weeks either side; only the days on screen count.
        var place = CalendarLoading.Place(7, 13, week => week != 1, Metrics);

        Assert.False(place.Shown);
    }

    [Fact]
    public void AWeekBeforeTheAnchorIsCountedAsItsOwnWeek()
    {
        var place = CalendarLoading.Place(-2, 4, week => week == -1, Metrics);

        Assert.True(place.Shown);
    }

    [Fact]
    public void ThePillSitsAtTheTopOfTheHoursCentredOverTheDays()
    {
        var m = Metrics;

        var place = CalendarLoading.Place(0, 6, _ => true, m);

        // Below the headings and the all-day banner, whatever hour the day is scrolled to, and inset by
        // the ruler so it is centred over the days rather than the whole surface.
        Assert.True(m.BannerLanes > 0, "the case needs an all-day banner for the pill to sit below");
        Assert.Equal(Screen.HeaderHeight + m.BannerHeight + Screen.DividerHeight + CalendarLoading.PillGap, place.Top);
        Assert.Equal(Screen.Gutter, place.Left);
    }
}
