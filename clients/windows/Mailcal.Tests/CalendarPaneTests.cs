// The calendar pane's own state and drawing rules (docs/calendar-pane.md): the paged month is the
// only thing the client keeps, and it lets go when the grid's shown days change (rule 3); the band's
// ends, the day buttons' names and the arrow keys (rules 4 and 11); and today in the display zone.

using System.Globalization;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarPaneTests
{
    private static readonly GridPlace Week = new(CalendarLayout.Week, new DateOnly(2026, 11, 16), 7);

    [Fact]
    public void TheQueryCarriesTheGridAndToday()
    {
        var model = new CalendarPaneModel();
        model.Follow(Week);

        var query = model.Query(new DateOnly(2026, 11, 18), Week);

        Assert.Equal("2026-11-18", query.Today);
        Assert.Equal(CalendarLayout.Week, query.Layout);
        Assert.Equal("2026-11-16", query.Anchor);
        Assert.Equal(7u, query.Columns);
        Assert.Null(query.Month);
    }

    [Fact]
    public void APagedMonthRidesOnTheQueryWhileTheGridStays()
    {
        var model = new CalendarPaneModel();
        model.Follow(Week);
        model.Page("2026-12-01");

        Assert.False(model.Follow(Week));
        Assert.Equal("2026-12-01", model.Query(new DateOnly(2026, 11, 18), Week).Month);
    }

    [Theory]
    // Generated enums are internal, and a public theory cannot take one, so they arrive by number.
    [InlineData((int)CalendarLayout.Week, 2026, 11, 23, 7u)]
    [InlineData((int)CalendarLayout.Day, 2026, 11, 16, 1u)]
    [InlineData((int)CalendarLayout.Week, 2026, 11, 16, 5u)]
    public void TheGridMovingLetsGoOfThePagedMonth(int layout, int year, int month, int day, uint columns)
    {
        var model = new CalendarPaneModel();
        model.Follow(Week);
        model.Page("2027-01-01");

        Assert.True(model.Follow(new GridPlace((CalendarLayout)layout, new DateOnly(year, month, day), columns)));
        Assert.Null(model.PagedMonth);
    }

    [Theory]
    [InlineData((int)ShownSpan.Only, true, true)]
    [InlineData((int)ShownSpan.Start, true, false)]
    [InlineData((int)ShownSpan.Middle, false, false)]
    [InlineData((int)ShownSpan.End, false, true)]
    public void TheBandRoundsItsEndsWithinARow(int span, bool start, bool end) =>
        Assert.Equal(new BandEnds(start, end), CalendarPaneModel.Band((ShownSpan)span));

    [Fact]
    public void ADayOutsideTheGridWearsNoBand() => Assert.Null(CalendarPaneModel.Band(ShownSpan.Outside));

    private static readonly PaneDayWords Words = new("Today", "Selected", "Shown in the calendar");

    [Fact]
    public void ADaySaysItsFullDateThenTodayThenThatTheGridShowsIt()
    {
        var culture = CultureInfo.GetCultureInfo("en-GB");
        var day = new MiniDay("2026-11-18", 18, true, true, ShownSpan.Middle, false);

        Assert.Equal(
            "Wednesday, 18 November 2026, Today, Shown in the calendar",
            CalendarPaneModel.DayName(day, culture, Words));
        Assert.Equal(
            "Thursday, 19 November 2026",
            CalendarPaneModel.DayName(day with { Date = "2026-11-19", IsToday = false, Shown = ShownSpan.Outside }, culture, Words));
    }

    [Fact]
    public void TheFocusDaySaysItIsSelectedAfterToday()
    {
        var culture = CultureInfo.GetCultureInfo("en-GB");
        var day = new MiniDay("2026-11-18", 18, true, true, ShownSpan.Middle, true);

        Assert.Equal(
            "Wednesday, 18 November 2026, Today, Selected, Shown in the calendar",
            CalendarPaneModel.DayName(day, culture, Words));
        Assert.Equal(
            "Thursday, 19 November 2026, Selected",
            CalendarPaneModel.DayName(day with { Date = "2026-11-19", IsToday = false, Shown = ShownSpan.Outside }, culture, Words));
    }

    [Theory]
    [InlineData((int)RecurrenceWeekday.Monday, "en-GB", "M")]
    [InlineData((int)RecurrenceWeekday.Sunday, "en-GB", "S")]
    [InlineData((int)RecurrenceWeekday.Wednesday, "nl-NL", "W")]
    [InlineData((int)RecurrenceWeekday.Thursday, "de-DE", "D")]
    public void EachColumnIsHeadedByItsWeekdaysInitial(int day, string culture, string expected) =>
        Assert.Equal(expected, CalendarPaneModel.WeekdayInitial((RecurrenceWeekday)day, CultureInfo.GetCultureInfo(culture)));

    [Fact]
    public void TheTitleIsTheMonthOfItsFirstDay()
    {
        var month = new MiniMonth("2026-11-01", "2026-10-01", "2026-12-01", [], []);
        Assert.Equal("November 2026", CalendarPaneModel.MonthTitle(month, CultureInfo.GetCultureInfo("en-GB")));
    }

    [Theory]
    [InlineData(10, 1, 0, 11)]
    [InlineData(10, -1, 0, 9)]
    [InlineData(10, 0, 1, 17)]
    [InlineData(10, 0, -1, 3)]
    // At an edge the focus stays where it is.
    [InlineData(0, -1, 0, 0)]
    [InlineData(3, 0, -1, 3)]
    [InlineData(41, 1, 0, 41)]
    [InlineData(38, 0, 1, 38)]
    public void ArrowKeysMoveBetweenDays(int from, int columns, int rows, int expected) =>
        Assert.Equal(expected, CalendarPaneModel.Move(from, columns, rows));

    [Theory]
    [InlineData(240, 30)]
    [InlineData(400, 36)]
    [InlineData(0, 0)]
    public void ACellIsAnEighthOfThePaneAndStaysRound(double width, double expected) =>
        Assert.Equal(expected, CalendarPaneModel.CellSize(width));

    [Fact]
    public void TodayIsTheDisplayZonesDate()
    {
        // 23:30 UTC on the 18th is already the 19th in Amsterdam and still the 18th in New York.
        var now = new DateTimeOffset(2026, 11, 18, 23, 30, 0, TimeSpan.Zero);

        Assert.Equal(new DateOnly(2026, 11, 19), DateOnly.FromDateTime(TimeZones.WallClock("Europe/Amsterdam", now)));
        Assert.Equal(new DateOnly(2026, 11, 18), DateOnly.FromDateTime(TimeZones.WallClock("America/New_York", now)));
    }

    [Fact]
    public void AnUnknownZoneFallsBackToThisMachinesClock()
    {
        var now = new DateTimeOffset(2026, 11, 18, 12, 0, 0, TimeSpan.Zero);
        Assert.Equal(now.ToLocalTime().DateTime, TimeZones.WallClock("", now));
    }
}
