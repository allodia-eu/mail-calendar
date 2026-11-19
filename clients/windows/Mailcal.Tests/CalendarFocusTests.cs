// The Windows calendar's focus date (docs/calendar.md, "The focus date"): it moves only through the
// core, and a strip a gesture took elsewhere takes it along. How far a step goes is the core's, so the
// fake below only shifts the date and says what it was asked; the rules themselves are pinned in
// mailcal-viewmodel's focus tests.
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarFocusTests
{
    private static readonly DateOnly Today = new(2026, 11, 19); // a Thursday

    // Weeks start on Monday here; the real one is the core's.
    private static DateOnly WeekStart(DateOnly date) => date.AddDays(-(((int)date.DayOfWeek + 6) % 7));

    private sealed class FakeCore : ICalendarFocusCore
    {
        internal DateOnly Held { get; private set; } = Today;

        internal List<(CalendarLayout Layout, int Steps)> Steps { get; } = [];

        public void Set(DateOnly date) => Held = date;

        public DateOnly Step(CalendarLayout layout, int steps, DateOnly today)
        {
            Steps.Add((layout, steps));
            Held = layout == CalendarLayout.Month ? Held.AddMonths(steps) : Held.AddDays(7 * steps);
            return Held;
        }
    }

    [Fact]
    public void AStepIsTheCoresAndTheClientKeepsWhereItLanded()
    {
        var core = new FakeCore();
        var focus = new CalendarFocus(core, Today);

        var landed = focus.Step(CalendarMode.Month, -4, Today);

        Assert.Equal([(CalendarLayout.Month, -4)], core.Steps);
        Assert.Equal(new DateOnly(2026, 7, 19), landed);
        Assert.Equal(landed, focus.Focus);
    }

    [Fact]
    public void ASetReachesTheCore()
    {
        var core = new FakeCore();
        var focus = new CalendarFocus(core, Today);

        focus.Set(new DateOnly(2026, 7, 1));

        Assert.Equal(new DateOnly(2026, 7, 1), core.Held);
        Assert.Equal(new DateOnly(2026, 7, 1), focus.Focus);
    }

    [Fact]
    public void ARestThatStillShowsTheFocusLeavesItAlone()
    {
        // After a step, the strip lands on the week the focus is in: nothing to follow.
        var core = new FakeCore();
        var focus = new CalendarFocus(core, Today);

        focus.Rest(CalendarMode.Week, WeekStart(Today), 7, Today, WeekStart);

        Assert.Empty(core.Steps);
        Assert.Equal(Today, focus.Focus);
    }

    [Fact]
    public void AWeekendFocusIsInTheWorkWeekItsWeekDraws()
    {
        // Monday to Friday does not draw the Saturday, but the Saturday is in that week: switching to
        // the work week must not move the focus.
        var saturday = new DateOnly(2026, 11, 21);
        var core = new FakeCore();
        var focus = new CalendarFocus(core, Today);
        focus.Set(saturday);

        focus.Rest(CalendarMode.WorkWeek, WeekStart(saturday), 5, Today, WeekStart);

        Assert.Equal(saturday, focus.Focus);
    }

    [Fact]
    public void ASwipeOfAWeekTakesTheFocusAWeekThroughTheCore()
    {
        var core = new FakeCore();
        var focus = new CalendarFocus(core, Today);

        focus.Rest(CalendarMode.Week, WeekStart(Today).AddDays(14), 7, Today, WeekStart);

        Assert.Equal([(CalendarLayout.Week, 2)], core.Steps);
        Assert.Equal(Today.AddDays(14), focus.Focus);
    }

    [Fact]
    public void AWeekRestingAcrossASeamTakesTheFocusToTheWeekInTheMiddle()
    {
        // Thursday to Wednesday, three weeks back: the middle of the screen is Sunday, in the week the
        // Thursday begins.
        var core = new FakeCore();
        var focus = new CalendarFocus(core, Today);
        var first = WeekStart(Today).AddDays(-21 + 3);

        focus.Rest(CalendarMode.Week, first, 7, Today, WeekStart);

        Assert.Equal([(CalendarLayout.Week, -3)], core.Steps);
    }

    [Fact]
    public void ADayScrolledAwayFromMovesTheFocusToTheDayShown()
    {
        var core = new FakeCore();
        var focus = new CalendarFocus(core, Today);

        focus.Rest(CalendarMode.Day, Today.AddDays(5), 1, Today, WeekStart);

        Assert.Equal(Today.AddDays(5), focus.Focus);
        Assert.Equal(Today.AddDays(5), core.Held);
        Assert.Empty(core.Steps);
    }

    [Fact]
    public void TheMonthAndTheAgendaHaveNoStripToFollow()
    {
        var core = new FakeCore();
        var focus = new CalendarFocus(core, Today);

        focus.Rest(CalendarMode.Month, Today.AddDays(40), 7, Today, WeekStart);
        focus.Rest(CalendarMode.Agenda, Today.AddDays(40), 7, Today, WeekStart);

        Assert.Equal(Today, focus.Focus);
    }
}
