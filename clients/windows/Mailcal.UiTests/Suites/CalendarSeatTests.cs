// Where the week opens when the calendar was last left on a shape without columns (docs/calendar.md
// §3, "How the aligned week is framed when it opens"): on the week's first day, never on today.
//
// WHY HERE. The pager aligns a seat only through the week-start function it was built with, and which
// pager a launch builds is decided in CalendarView, which Mailcal.Tests cannot link. A launch that
// opened on the agenda or the month kept a pager with no week start at all, so picking Week opened on
// today: a Tuesday under the first heading.
//
// WHY THE HARNESS. The shape is the core's and persists, which the showcase never does, so only a
// harness launch can open on the agenda the previous one left. Its week starts on Monday, the default.
//
// NOTHING BELOW MATCHES A LOCALISED STRING: a day heading ends in its day of the month, and that is
// what is compared.

using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed partial class CalendarSeatTests(HarnessApp harness)
{
    [HarnessFact]
    public void ALaunchLeftOnTheAgendaOpensTheWeekOnItsFirstDay() => harness.Case(nameof(ALaunchLeftOnTheAgendaOpensTheWeekOnItsFirstDay), app =>
    {
        var today = DateOnly.FromDateTime(DateTime.Now);
        Assert.SkipWhen(
            today.DayOfWeek == DayOfWeek.Monday,
            "on a Monday the week's first day is today, so a seat on today cannot be told from an aligned one");
        var monday = today.AddDays(-(((int)today.DayOfWeek + 6) % 7));

        new CalendarFlow(app).ShowAgenda();
        harness.RelaunchOnHarness();
        app = harness.App;
        new CalendarFlow(app).SetShape(Filter.ById("CalendarViewWeek", "MenuItem"));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(800));

        var first = FirstColumn(app);
        Assert.True(
            DayOf(first) == monday.Day,
            $"the week opened on '{first.Name}', where its first day is Monday {monday:yyyy-MM-dd}: the launch built its pager without the core's week start");
    });

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
