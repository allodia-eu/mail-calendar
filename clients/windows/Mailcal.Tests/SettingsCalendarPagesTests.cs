// Settings → Calendar's default list and Settings → Videoconferencing, drawn from the core's
// records: when the Jitsi field says its address was refused, and which changes redraw a group
// rather than move its selection in place.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SettingsCalendarPagesTests
{
    private static VideoconferencingPage Page(string? stored, string? refused) => new(stored, refused, []);

    [Fact]
    public void TheRefusalShowsWhileTheFieldHoldsTheRefusedText()
    {
        var page = Page(null, "http://meet.test.local");
        Assert.True(SettingsCalendarPages.JitsiRefused("http://meet.test.local", page));
        Assert.False(SettingsCalendarPages.JitsiRefused("http://meet.test.loca", page));
        Assert.False(SettingsCalendarPages.JitsiRefused(" http://meet.test.local", page));
    }

    [Fact]
    public void NothingRefusedShowsNoRefusal()
    {
        Assert.False(SettingsCalendarPages.JitsiRefused("https://meet.test.local", Page("https://meet.test.local", null)));
        Assert.False(SettingsCalendarPages.JitsiRefused(string.Empty, Page(null, null)));
        Assert.False(SettingsCalendarPages.JitsiRefused(string.Empty, null));
    }

    private static CalendarRow Calendar(string account, string id, bool isDefault = false)
    {
        var swatch = new Swatch("#336699", "#ffffff", "#224466");
        return new CalendarRow(
            account, account + "@test.local", id, id, new CalendarColor("#336699", swatch, swatch),
            true, true, false, isDefault);
    }

    private static NewEventCalendars Offered(bool named, params CalendarRow[] rows) => new(
        rows.GroupBy(row => row.Account)
            .Select(group => new CalendarGroup(group.Key, group.Key + "@test.local", true, group.ToArray()))
            .ToArray(),
        named);

    [Fact]
    public void AnotherDefaultMovesTheCheckRatherThanRedrawing()
    {
        Assert.Equal(
            SettingsCalendarPages.DefaultCalendarsFingerprint(Offered(false, Calendar("a", "work", isDefault: true), Calendar("a", "home"))),
            SettingsCalendarPages.DefaultCalendarsFingerprint(Offered(false, Calendar("a", "work"), Calendar("a", "home", isDefault: true))));
    }

    [Fact]
    public void ACalendarArrivingOrTheHeadingsChangingRedraws()
    {
        var one = SettingsCalendarPages.DefaultCalendarsFingerprint(Offered(false, Calendar("a", "work")));
        Assert.NotEqual(one, SettingsCalendarPages.DefaultCalendarsFingerprint(Offered(false, Calendar("a", "work"), Calendar("a", "home"))));
        Assert.NotEqual(one, SettingsCalendarPages.DefaultCalendarsFingerprint(Offered(true, Calendar("a", "work"))));
        Assert.NotEqual(one, SettingsCalendarPages.DefaultCalendarsFingerprint(Offered(false)));
        Assert.NotEqual(SettingsCalendarPages.DefaultCalendarsFingerprint(Offered(false)), SettingsCalendarPages.DefaultCalendarsFingerprint(null));
    }

    [Fact]
    public void AnotherMeetingChoiceMovesThePickerRatherThanRedrawing()
    {
        MeetingOption[] choices = [new("jitsi", "Jitsi"), new("teams", "Microsoft Teams", "a@test.local")];
        Assert.Equal(
            SettingsCalendarPages.MeetingDefaultsFingerprint([new MeetingDefault("a", "a@test.local", choices, "jitsi")]),
            SettingsCalendarPages.MeetingDefaultsFingerprint([new MeetingDefault("a", "a@test.local", choices, null)]));
        Assert.NotEqual(
            SettingsCalendarPages.MeetingDefaultsFingerprint([new MeetingDefault("a", "a@test.local", choices, null)]),
            SettingsCalendarPages.MeetingDefaultsFingerprint([new MeetingDefault("a", "a@test.local", choices[..1], null)]));
    }
}
