// When a Settings signal redraws the agenda and the grid: only when something they are drawn with
// moved. A change on an account's page moves none of it, and redrawing the calendar for each held
// the UI thread long enough that the control pressed never showed it was pressed.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarSettingsRulesTests
{
    private static readonly DisplaySettings Display =
        new(WeekStart.Monday, TimeFormat.TwentyFourHour, Appearance.System, true, 12, CalendarLayout.Week);

    private static CalendarRow Calendar(bool visible = true, bool canWrite = true)
    {
        var swatch = new Swatch("#336699", "#ffffff", "#224466");
        return new CalendarRow(
            "acct-1", "me@example.org", "work", "Work", new CalendarColor("#336699", swatch, swatch),
            visible, canWrite, false, canWrite);
    }

    private static string Drawn(
        string zone = "Europe/Amsterdam", bool use24Hour = true, DisplaySettings? display = null, CalendarRow? calendar = null) =>
        CalendarSettingsRules.Fingerprint(zone, use24Hour, display ?? Display, [calendar ?? Calendar()]);

    [Fact]
    public void ASignalThatMovedNothingTheCalendarIsDrawnWithLeavesIt() => Assert.Equal(Drawn(), Drawn());

    [Fact]
    public void EachThingTheCalendarIsDrawnWithRedrawsIt()
    {
        var drawn = Drawn();
        Assert.NotEqual(drawn, Drawn(zone: "Europe/Lisbon"));
        Assert.NotEqual(drawn, Drawn(use24Hour: false));
        Assert.NotEqual(drawn, Drawn(display: Display with { WeekStart = WeekStart.Sunday }));
        Assert.NotEqual(drawn, Drawn(display: Display with { VisibleHours = 16 }));
        Assert.NotEqual(drawn, Drawn(calendar: Calendar(visible: false)));
        // Whether a new event can be made is read off the calendars' rights.
        Assert.NotEqual(drawn, Drawn(calendar: Calendar(canWrite: false)));
        Assert.NotEqual(drawn, CalendarSettingsRules.Fingerprint("Europe/Amsterdam", true, Display, []));
    }
}
