// The calendar pane beside the grid (docs/calendar-pane.md), on the showcase, whose in-memory seed
// draws a week with events on more than one calendar. Mailcal.Tests holds the paged month and the
// drawing rules; what only the running app can say is whether the pane reaches the screen in the
// folder pane's place, whether a calendar's tick reaches the grid, whether a pressed day moves it,
// and whether Mail leads back. The cases leave the calendars as they found them.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Trait("Dataset", "showcase")]
public sealed class CalendarPaneTests(CalendarShowcaseApp showcase) : IClassFixture<CalendarShowcaseApp>
{
    private static string Shown => Catalog.English("a11y_calendar_pane_day_shown");

    /// <summary>The folder tree's rows the calendar pane stands in for: the All Accounts group and the
    /// unified Inbox under it, which every store with an account draws.</summary>
    internal static Filter[] TreeRows(App app) =>
    [
        Filter.Named(Catalog.English("sidebar_all_accounts"), "ListItem") with { Window = app.Mailbox.Hwnd, OnScreen = true },
        Filter.Named(Catalog.English("folder_inbox"), "ListItem") with { Window = app.Mailbox.Hwnd, OnScreen = true },
    ];

    /// <summary>On the calendar the pane, and nothing of the folder tree, fills the column; Mail brings
    /// the tree back (rule 1). Shared by the showcase's launch and the harness's, whose window shows the
    /// shell only once the core has booted.</summary>
    internal static void TheTreeStepsAsideForThePane(App app)
    {
        new CalendarFlow(app).ShowCalendar();
        Pane(app);
        foreach (var row in TreeRows(app))
        {
            Assert.True(app.Agent.WaitGone(row, TimeSpan.FromSeconds(5)), $"'{row.Name}' of the folder tree is on screen beside the calendar pane");
        }

        app.Agent.Invoke(app.Agent.Find(Filter.ById("NavMail")) ?? throw new InvalidOperationException("no Mail destination beside the calendar"));
        foreach (var row in TreeRows(app))
        {
            Assert.True(app.Agent.Wait(row, TimeSpan.FromSeconds(10)) is not null, $"'{row.Name}' did not come back with mail");
        }
    }

    internal static Element Pane(App app) =>
        app.Agent.Wait(Filter.ById("CalendarPaneMonthTitle") with { OnScreen = true }, App.Patience)
        ?? throw new InvalidOperationException("no calendar pane within 30 s of the calendar coming up");

    private static Element Grid(App app) =>
        app.Agent.Wait(Filter.ById("CalendarGrid") with { OnScreen = true }, App.Patience)
        ?? throw new InvalidOperationException("no CalendarGrid within 30 s");

    private static List<Element> Days(App app) =>
        [.. app.Agent.FindAll(new Filter { Type = "Button" }).Where(e => e.Id.StartsWith("CalendarPaneDay_", StringComparison.Ordinal))];

    private static List<Element> Calendars(App app) =>
        [.. app.Agent.FindAll(new Filter { Type = "CheckBox" }).Where(e => e.Id.StartsWith("CalendarPaneCalendar_", StringComparison.Ordinal))];

    // The grid's events, each spoken as "title, time, calendar" and then whatever else it is.
    private static List<Element> Events(App app, Element grid) =>
        [.. app.Agent.FindAll(new Filter { Under = grid }).Where(e => e.Class == "CalendarItem")];

    [OfflineFact]
    public void ThePaneStandsInTheFolderPanesPlace() => showcase.Case(nameof(ThePaneStandsInTheFolderPanesPlace), app =>
    {
        new CalendarFlow(app).ShowCalendar();
        var title = Pane(app);
        Assert.False(string.IsNullOrEmpty(title.Name), "the mini month names its month");

        // Top to bottom: the mini month level with the calendar's own heading, the calendars, then the
        // destinations (rule 1). A pane sized to its content sits on the destinations instead, with the
        // column above it empty.
        var heading = app.Agent.Find(Filter.ById("CalendarPeriod")) ?? throw new InvalidOperationException("the calendar names no period");
        Assert.True(Math.Abs(title.Y - heading.Y) < 2 * Math.Max(title.Height, heading.Height),
            $"the mini month starts at y={title.Y}, the calendar's heading at y={heading.Y}: the pane does not fill the column from the top");
        var mail = app.Agent.Find(Filter.ById("NavMail")) ?? throw new InvalidOperationException("no Mail destination");
        Assert.True(Calendars(app).All(row => row.Y > title.Y && row.Y < mail.Y), "the calendars are not between the mini month and the destinations");
        foreach (var id in new[] { "CalendarPanePrevious", "CalendarPaneNext", "NavMail", "NavCalendar", "NavContacts" })
        {
            Assert.True(app.Agent.Find(new Filter { Id = id, OnScreen = true }) is not null, $"#{id} is not on screen beside the calendar");
        }

        var days = Days(app);
        Assert.Equal(42, days.Count);
        Assert.Contains(days, d => d.Name.Contains(", " + Catalog.English("calendar_today"), StringComparison.Ordinal));
        Assert.Contains(days, d => d.Name.Contains(Shown, StringComparison.Ordinal));
        Assert.NotEmpty(Calendars(app));

        // The accounts and folders step aside rather than standing above the pane.
        Assert.True(app.Agent.Find(Filter.ById("MenuItemsScrollViewer") with { OnScreen = true }) is null,
            "the folder tree is still on screen while the calendar pane is");
    });

    [OfflineFact]
    public void UntickingACalendarTakesItsEventsOffTheGrid() => showcase.Case(nameof(UntickingACalendarTakesItsEventsOffTheGrid), app =>
    {
        new CalendarFlow(app).SetShape(Filter.ById("CalendarViewWeek", "MenuItem"));
        Pane(app);
        var grid = Grid(app);
        Assert.True(App.Eventually(() => Events(app, grid).Count > 0, TimeSpan.FromSeconds(10)), "the showcase week draws no events");

        // A calendar with an event in this week: the row is named "calendar, account".
        var events = Events(app, grid);
        var row = Calendars(app).FirstOrDefault(r => events.Any(e => Names(e, CalendarOf(r))));
        Assert.True(row is not null, "no calendar in the pane names an event on the grid");
        var calendar = CalendarOf(row!);
        bool Drawn() => Events(app, grid).Any(e => Names(e, calendar));

        try
        {
            app.Agent.SetToggle(row!, false);
            Assert.True(App.Eventually(() => !Drawn(), TimeSpan.FromSeconds(10)), $"'{calendar}' is unticked and its events are still on the grid");
            Assert.True(App.Eventually(() => app.Agent.Find(Filter.ById(row!.Id)) is { } again && app.Agent.Read(again).Active == false, TimeSpan.FromSeconds(5)),
                "the row did not redraw unticked from the core's answer");
        }
        finally
        {
            if (app.Agent.Find(Filter.ById(row!.Id)) is { } again && app.Agent.Read(again).Active == false)
            {
                app.Agent.SetToggle(again, true);
            }
        }
        Assert.True(App.Eventually(Drawn, TimeSpan.FromSeconds(10)), $"ticking '{calendar}' again did not bring its events back");
    });

    [OfflineFact]
    public void PressingADayMovesTheGridToIt() => showcase.Case(nameof(PressingADayMovesTheGridToIt), app =>
    {
        var flow = new CalendarFlow(app);
        flow.SetShape(Filter.ById("CalendarViewWeek", "MenuItem"));
        Pane(app);
        // The furthest day the grid does not show, in the mini month's last row.
        var target = Days(app).Last(d => !d.Name.Contains(Shown, StringComparison.Ordinal));
        try
        {
            app.Agent.Invoke(target);
            Assert.True(
                App.Eventually(() => app.Agent.Find(Filter.ById(target.Id))?.Name.Contains(Shown, StringComparison.Ordinal) == true, TimeSpan.FromSeconds(10)),
                $"'{target.Name}' was pressed and the grid beside the pane does not show it");
        }
        finally
        {
            // Back to today's week, for whatever runs next.
            flow.SetShape(Filter.ById("CalendarViewWeek", "MenuItem"));
        }
    });

    [OfflineFact]
    public void TheFolderTreeStepsAsideForThePaneAndMailBringsItBack() =>
        showcase.Case(nameof(TheFolderTreeStepsAsideForThePaneAndMailBringsItBack), TheTreeStepsAsideForThePane);

    [OfflineFact]
    public void MailLeadsBackToTheMailList() => showcase.Case(nameof(MailLeadsBackToTheMailList), app =>
    {
        new CalendarFlow(app).ShowCalendar();
        Pane(app);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("NavMail")) ?? throw new InvalidOperationException("no Mail destination beside the calendar"));

        Assert.True(app.Agent.WaitGone(Filter.ById("CalendarPaneMonthTitle"), TimeSpan.FromSeconds(10)), "the calendar pane stayed after Mail was pressed");
        Assert.True(app.Agent.Wait(Filter.ById("MenuItemsScrollViewer") with { OnScreen = true }, TimeSpan.FromSeconds(10)) is not null,
            "the accounts and folders did not come back with mail");
        Assert.True(app.RowsList().Rendered, "Mail did not return to the mail list");
    });

    // Whether an event's spoken name names the calendar: third, and possibly last.
    private static bool Names(Element e, string calendar) =>
        e.Name.EndsWith(", " + calendar, StringComparison.Ordinal) || e.Name.Contains(", " + calendar + ",", StringComparison.Ordinal);

    // The calendar's name out of a row's "calendar, account".
    private static string CalendarOf(Element row)
    {
        var at = row.Name.LastIndexOf(", ", StringComparison.Ordinal);
        return at < 0 ? row.Name : row.Name[..at];
    }
}

/// <summary>The same on the harness, whose launch boots the core before it shows the shell: the pane
/// has to find its place in a NavigationView that was not drawn when the window was built.</summary>
[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class CalendarPaneHarnessTests(HarnessApp harness)
{
    [HarnessFact]
    public void TheFolderTreeStepsAsideForThePaneAfterABoot() =>
        harness.Case(nameof(TheFolderTreeStepsAsideForThePaneAfterABoot), CalendarPaneTests.TheTreeStepsAsideForThePane);
}
