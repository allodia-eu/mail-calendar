// The calendar's drawn week grid is reachable by id. The id once sat on the bare Grid the surface is
// added into, and a layout panel has no peer, so it reached nothing on any display and a wait for it
// could only time out; nothing noticed because nothing asked. The month surface overrides no
// automation peer, so it is in no tree at all (docs/calendar.md §7), and has no id to assert. New
// event, drawn over the same week, is disabled while the core offers no calendar to put one on.
//
// The showcase, because the question is whether a handle reaches a drawn element, and the in-memory
// seed draws a week with events in it. Every handle is an AutomationId. The cases run in the order
// written: the last puts the week away.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase opened on its calendar screen.</summary>
public sealed class CalendarShowcaseApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_SHOWCASE_SCREEN"] = "calendar" };
}

[Trait("Dataset", "showcase")]
public sealed class CalendarSurfacesTests(CalendarShowcaseApp showcase) : IClassFixture<CalendarShowcaseApp>
{
    // The grid is the whole detail pane when it is drawn at all; the floor only separates drawn from
    // a peer with an empty rectangle.
    private const double SurfaceFloorDip = 200;

    private static Element Grid(App app) =>
        app.Agent.Wait(Filter.ById("CalendarGrid") with { OnScreen = true }, App.Patience)
        ?? throw new InvalidOperationException("no CalendarGrid within 30 s: an id on the host Grid rather than the surface reaches nothing");

    [OfflineFact]
    public void TheWeekGridIsReachableByIdAndDrawn() => showcase.Case(nameof(TheWeekGridIsReachableByIdAndDrawn), app =>
    {
        var grid = Grid(app);
        Assert.True(grid.Rendered && grid.Height > SurfaceFloorDip * Input.Scale(grid.Window),
            $"the grid drew {grid.Height}px tall; it is the detail pane, so anything this small is not the surface");
    });

    [OfflineFact]
    public void NewEventIsDisabledWhereNoCalendarTakesANewEvent() => showcase.Case(nameof(NewEventIsDisabledWhereNoCalendarTakesANewEvent), app =>
    {
        // The showcase's calendar provider advertises no calendar writes, so the core's
        // NewEventCalendars is empty and the button is drawn disabled, never hidden. The harness
        // suites press it, and an invoke waits for it to be enabled, so they hold the other side.
        var grid = Grid(app);
        Assert.True(App.Eventually(() => app.Agent.FindAll(new Filter { Under = grid }).Any(e => e.Class == "CalendarItem"), TimeSpan.FromSeconds(10)),
            "the showcase week draws no events, so the calendar has not loaded and the button's state proves nothing");
        var add = app.Agent.Wait(Filter.ById("CalendarNewEvent", "Button") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("no New event on the calendar: it is disabled while nothing takes a new event, never hidden");
        Assert.False(add.Enabled, "New event is enabled on the showcase, whose calendars take no new event");
        Assert.False(App.Eventually(() => app.Agent.Find(Filter.ById("CalendarNewEvent", "Button")) is { Enabled: true }, TimeSpan.FromSeconds(3)),
            "New event became enabled on the showcase, whose calendars take no new event");
    });

    [OfflineFact]
    public void PickingADifferentZoomActuallyPutsTheWeekGridAway() => showcase.Case(nameof(PickingADifferentZoomActuallyPutsTheWeekGridAway), app =>
    {
        // What stops the case above passing on a surface that is always there. Month, because it is
        // the one zoom that swaps the surface rather than redrawing this one (§2).
        Grid(app);
        new CalendarFlow(app).SetShape(Filter.ById("CalendarViewMonth", "MenuItem"));
        Assert.True(app.Agent.WaitGone(Filter.ById("CalendarGrid"), TimeSpan.FromSeconds(15)),
            "the week grid is still in the tree after switching to Month: the view did not change, or the id outlives the surface it names");
    });
}
