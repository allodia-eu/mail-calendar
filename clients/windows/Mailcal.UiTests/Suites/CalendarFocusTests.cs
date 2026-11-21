// The calendar keeps one focus date, held by the core, and draws every view around it
// (docs/calendar.md, "The focus date"). Paging the month moves it a month at a time on the same day of
// the month; the week then opens on the week holding it, and switching views never moves it.
//
// WHY HERE. Which week a menu choice opens is decided in CalendarView, between the core's focus and
// the pager, which Mailcal.Tests cannot link; the rules themselves are the core's, pinned in its own
// tests.
//
// The showcase, because only the client's wiring is in question and its week starts on Monday, the
// default. NOTHING BELOW MATCHES A LOCALISED STRING: a day heading ends in its day of the month, and
// that is what is compared.

using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "showcase")]
public sealed partial class CalendarFocusTests(CalendarShowcaseApp showcase) : IClassFixture<CalendarShowcaseApp>
{
    private const int MonthsBack = 4;

    [OfflineFact]
    public void TheWeekAfterAPagedMonthIsTheWeekHoldingTheFocusAndSurvivesARoundTrip() => showcase.Case(nameof(TheWeekAfterAPagedMonthIsTheWeekHoldingTheFocusAndSurvivesARoundTrip), app =>
    {
        // Today first, so the focus starts where a launch puts it.
        new CalendarFlow(app).SetShape(Filter.ById("CalendarViewMonth", "MenuItem"));
        for (var i = 0; i < MonthsBack; i++)
        {
            app.Agent.Invoke(app.Agent.Find(Filter.ById("CalendarPrev", "Button"))
                ?? throw new InvalidOperationException("the month offers no previous-month button"));
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(200));
        }

        // A month steps on the same day of the month, the last day of a shorter month standing in.
        var focus = DateOnly.FromDateTime(DateTime.Now).AddMonths(-MonthsBack);
        var monday = focus.AddDays(-(((int)focus.DayOfWeek + 6) % 7));

        Pick(app, "CalendarViewWeek");
        var first = DayOf(FirstColumn(app));
        Assert.True(first == monday.Day,
            $"the week opened on the {first}th, where the focus {focus:yyyy-MM-dd} is in the week of Monday {monday:yyyy-MM-dd}: a view switch moved the focus");

        Pick(app, "CalendarViewMonth");
        Pick(app, "CalendarViewWeek");
        var again = DayOf(FirstColumn(app));
        Assert.True(again == first,
            $"week, month, week came back on the {again}th rather than the {first}th: the round trip moved the focus");
    });

    // A choice from the view menu seats the grid on the focus's week, which moves the pager's origin;
    // Today moves it back. The grid's cached weeks are placed from that origin, so a Today that kept
    // them drew the focus's week again under today's.
    [OfflineFact]
    public void TodayComesBackToThisWeekAfterAChoiceSeatedTheGridOnTheFocus() => showcase.Case(nameof(TodayComesBackToThisWeekAfterAChoiceSeatedTheGridOnTheFocus), app =>
    {
        new CalendarFlow(app).SetShape(Filter.ById("CalendarViewWeek", "MenuItem"));
        for (var i = 0; i < 5; i++)
        {
            app.Agent.Invoke(app.Agent.Find(Filter.ById("CalendarNext", "Button"))
                ?? throw new InvalidOperationException("the week offers no next-week button"));
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(200));
        }
        Pick(app, "CalendarViewWeek");
        var today = DateOnly.FromDateTime(DateTime.Now);
        var monday = today.AddDays(-(((int)today.DayOfWeek + 6) % 7));
        var seated = DayOf(FirstColumn(app));
        Assert.True(seated == monday.AddDays(35).Day,
            $"the menu's week opens on the focus five weeks on, the {monday.AddDays(35).Day}th; it opened on the {seated}th");

        app.Agent.Invoke(app.Agent.Find(Filter.ById("CalendarToday", "Button")) ?? throw new InvalidOperationException("no Today button"));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(800));
        var first = DayOf(FirstColumn(app));
        Assert.True(first == monday.Day,
            $"Today drew the week from the {first}th rather than this week's Monday, the {monday.Day}th: the grid kept the focus's week");
    });

    /// <summary>Chooses a shape from the view menu, and nothing else: no Today, which would put the
    /// focus back on today.</summary>
    private static void Pick(App app, string id)
    {
        var item = Filter.ById(id, "MenuItem");
        app.Agent.Invoke(app.Agent.Find(Filter.ById("CalendarViewMenu", "Button"))
            ?? throw new InvalidOperationException("the calendar has no view menu"));
        app.Agent.Invoke(app.Agent.Wait(item, TimeSpan.FromSeconds(10)) ?? throw new InvalidOperationException($"the view menu offers no #{id}"));
        app.Agent.WaitGone(item, TimeSpan.FromSeconds(5));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(800));
    }

    /// <summary>The leftmost day heading on screen. The grid's peer also lists the weeks either side,
    /// which lie left and right of it.</summary>
    private static Element FirstColumn(App app)
    {
        var grid = app.Agent.Wait(Filter.ById("CalendarGrid") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("the week grid did not come up");
        return app.Agent.FindAll(new Filter { Type = "HeaderItem", Under = grid })
            .Where(heading => heading.X >= grid.X && heading.X < grid.X + grid.Width)
            .MinBy(heading => heading.X)
            ?? throw new InvalidOperationException("the week grid names no day on screen");
    }

    private static int DayOf(Element heading) =>
        int.Parse(TrailingNumber().Match(heading.Name).Value, System.Globalization.CultureInfo.InvariantCulture);

    [GeneratedRegex(@"\d+$")]
    private static partial Regex TrailingNumber();
}
