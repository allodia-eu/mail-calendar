// Does a wheel move the calendar? Mailcal.Tests drives the gesture owner, the driver and the state
// machine, and never sees CalendarSurface, where the wheel is received and turned into a drawn
// frame. Every defect this was written for lived in that gap: a notch started the tick loop without
// marking the surface dirty, so nothing was drawn; the loop spent a frame on each restart; the
// travel was applied at once, so the grid teleported per notch and stood still between.
//
// The wheel is real input, with the window in front and the pointer on the grid, or the notches land
// on whatever is under it. The oracle is the period heading, which names the span the grid shows: it
// names a month, so it cannot see a day of travel, but it is the only position the grid publishes,
// and it is enough to say the wheel moves the grid, by how far it was turned rather than a page.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class CalendarWheelTests(ShowcaseApp showcase)
{
    private void Case(string name, Action<App> body) =>
        showcase.Case(name, app =>
        {
            new CalendarFlow(app).SetShape(Filter.ById("CalendarViewWeek", "MenuItem"));
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(800));
            body(app);
        });

    private static string Period(App app) => app.Agent.Find(Filter.ById("CalendarPeriod"))?.Name ?? "";

    /// <summary>Turns a horizontal wheel over the hour grid at a mouse's own cadence, then lets the
    /// pan come to rest. The point is placed off the header's chevrons, at the grid's top right.</summary>
    private static void Wheel(App app, int notches)
    {
        var next = app.Agent.Find(Filter.ById("CalendarNext")) ?? throw new InvalidOperationException("the calendar is not on screen");
        Input.BringToFront(next.Window);
        var scale = Input.Scale(next.Window);
        var (x, y) = ((int)(next.X - (100 * scale)), (int)(next.Y + next.Height + (200 * scale)));
        for (var i = 0; i < notches; i++)
        {
            Input.Wheel(x, y, 1, horizontal: true);
            Thread.Sleep(120);
        }
        Thread.Sleep(600);
    }

    /// <summary>The heading once it reads something other than <paramref name="from"/>: the grid comes
    /// to rest after the last notch, at a pace that is the machine's, not the gesture's.</summary>
    private static string WaitPeriodChange(App app, string from, int timeoutSec = 10)
    {
        App.Eventually(() => Period(app) != from, TimeSpan.FromSeconds(timeoutSec));
        return Period(app);
    }

    [OfflineFact]
    public void AHorizontalWheelMovesTheGrid() => Case(nameof(AHorizontalWheelMovesTheGrid), app =>
    {
        var before = Period(app);
        Assert.True(before.Length > 0, "the calendar names the span it shows");
        // How far the grid travels before a month heading changes depends on the date: a week inside
        // one month needs weeks of travel. So the wheel turns in batches, up to a bound no month
        // outlasts; a wheel that moves nothing still fails after the last.
        var after = before;
        for (var batch = 0; batch < 4 && after == before; batch++)
        {
            Wheel(app, 24);
            after = WaitPeriodChange(app, before, 4);
        }
        Assert.True(after != before, $"the grid still says '{after}' after 96 notches: the owner and the driver are content and the surface never draws");
    });

    [OfflineFact]
    public void ScrollingFurtherTravelsFurtherRatherThanTurningOnePage() => Case(nameof(ScrollingFurtherTravelsFurtherRatherThanTurningOnePage), app =>
    {
        var today = app.Agent.Find(Filter.ById("CalendarToday", "Button")) ?? throw new InvalidOperationException("no Today button");
        app.Agent.Invoke(today);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(800));
        var origin = Period(app);
        Wheel(app, 12);
        var shortTravel = Period(app);
        // Today animates back like any other travel, so the return is waited for.
        app.Agent.Invoke(today);
        Assert.True(App.Eventually(() => Period(app) == origin, TimeSpan.FromSeconds(10)), "Today puts the grid back where it started");
        Wheel(app, 48);
        // The leg that has to leave the month, which is what this case claims.
        var longTravel = WaitPeriodChange(app, origin);
        // A continuous strip, not a pager: four times the input goes four times as far, so the two
        // legs cannot land on one span. A page per gesture would.
        Assert.True(longTravel != shortTravel, $"12 notches and 48 both landed on '{shortTravel}': the grid pages by a fixed span instead of travelling as far as the wheel asked");
    });
}
