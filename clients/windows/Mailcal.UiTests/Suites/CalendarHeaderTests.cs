// The calendar's header names what is on screen. The month titles the header itself, while the week
// grid titles it only when its own title changes, so a grid returning to the days it last showed left
// the month's name over them: "July 2026" above the week of today.
//
// The showcase, because the question is the header against the grid, and both are the client's. The
// grid's automation name is its own title, so the case compares the two and matches no localised
// string.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "showcase")]
public sealed class CalendarHeaderTests(CalendarShowcaseApp showcase) : IClassFixture<CalendarShowcaseApp>
{
    private const int MonthsBack = 4;

    [OfflineFact]
    public void TheWeekAfterAPagedMonthIsTitledByTheWeek() => showcase.Case(nameof(TheWeekAfterAPagedMonthIsTitledByTheWeek), app =>
    {
        var flow = new CalendarFlow(app);
        flow.SetShape(Filter.ById("CalendarViewWeek", "MenuItem"));
        var week = Title(app);

        flow.SetShape(Filter.ById("CalendarViewMonth", "MenuItem"));
        for (var i = 0; i < MonthsBack; i++)
        {
            app.Agent.Invoke(app.Agent.Find(Filter.ById("CalendarPrev", "Button"))
                ?? throw new InvalidOperationException("the month offers no previous-month button"));
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(200));
        }
        var month = Header(app);
        Assert.NotEqual(week, month);

        flow.SetShape(Filter.ById("CalendarViewWeek", "MenuItem"));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(800));

        Assert.True(Header(app) == Title(app),
            $"the header reads '{Header(app)}' over a grid showing '{Title(app)}': the month's title outlived it");
    });

    /// <summary>The grid's own title, which its automation name is.</summary>
    private static string Title(App app) =>
        (app.Agent.Wait(Filter.ById("CalendarGrid") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("the week grid did not come up")).Name;

    private static string Header(App app) =>
        (app.Agent.Find(Filter.ById("CalendarPeriod"))
            ?? throw new InvalidOperationException("the calendar has no period title")).Name;
}
