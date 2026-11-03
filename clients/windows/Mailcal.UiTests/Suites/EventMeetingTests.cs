// An event's video meeting, in the agenda and in its detail (docs/conferencing.md).
//
// The showcase's Monday design sync carries a Teams meeting with its meeting ID and a dial-in
// number in every locale. It is reached through the agenda, which lists the whole week whatever day
// the suite runs on; the grid shows only the days around today. The agenda row's camera is hidden
// from UI Automation, so its name is what says the event has a meeting, and a plain event beside
// it is what stops that assertion passing on a name that always says it.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class EventMeetingTests(ShowcaseApp showcase)
{
    private const string WithMeeting = "Design sync";
    private const string Plain = "Gym";
    private const string DialIn = "+31 20 555 0100";

    private void Case(string name, Action<App, CalendarFlow> body) =>
        showcase.Case(name, app => body(app, new CalendarFlow(app)), app => app.CloseDialogs());

    private static string RowName(App app, Element agenda, string title) =>
        (app.Agent.Wait(new Filter { Type = "ListItem", Under = agenda, Has = Filter.Named(title, "Text") }, TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException($"the agenda lists no '{title}'")).Name;

    [OfflineFact]
    public void TheAgendaSaysWhichEventHasAVideoMeeting() => Case(nameof(TheAgendaSaysWhichEventHasAVideoMeeting), (app, flow) =>
    {
        var agenda = flow.ShowAgenda();
        var words = Catalog.Everywhere("event_video_meeting");
        var meeting = RowName(app, agenda, WithMeeting);
        Assert.True(words.Any(w => meeting.EndsWith(", " + w, StringComparison.Ordinal)),
            $"the design sync's row does not say it has a video meeting: '{meeting}'");
        var plain = RowName(app, agenda, Plain);
        Assert.False(words.Any(w => plain.Contains(w, StringComparison.Ordinal)),
            $"an event with no meeting says it has one: '{plain}'");
    });

    [OfflineFact]
    public void TheDetailOffersJoinUnderTheServicesName() => Case(nameof(TheDetailOffersJoinUnderTheServicesName), (app, flow) =>
    {
        var detail = flow.OpenDetail(WithMeeting);
        Assert.True(app.Agent.Wait(new Filter { Id = "EventJoin", Type = "Button", Under = detail }, TimeSpan.FromSeconds(10)) is not null,
            "the detail offers no Join");
        Assert.Equal("Microsoft Teams", app.Agent.Find(new Filter { Id = "EventMeetingService", Under = detail })?.Name);
        Assert.Equal("990 000 000 001", app.Agent.Find(new Filter { Id = "EventMeetingId", Under = detail })?.Name);
        Assert.True(app.Agent.Find(new Filter { Id = "EventMeetingCopyLink", Type = "Button", Under = detail }) is not null,
            "a meeting Join can open offers its link to copy");
    });

    // A row is a link only when the core sent it a `Uri`, and is text otherwise, so this is where
    // the core's optional URI is seen to reach the detail.
    [OfflineFact]
    public void TheDetailOffersTheDialInNumberToDialWithItsPin() => Case(nameof(TheDetailOffersTheDialInNumberToDialWithItsPin), (app, flow) =>
    {
        var detail = flow.OpenDetail(WithMeeting);
        Assert.True(app.Agent.Wait(Filter.Named(DialIn, "Hyperlink") with { Under = detail }, TimeSpan.FromSeconds(10)) is not null,
            $"the detail does not offer {DialIn} to dial");
        var pin = Catalog.English("event_meeting_pin").Replace("{pin}", "990 000 000#", StringComparison.Ordinal);
        Assert.True(app.Agent.Find(Filter.Named(pin, "Text") with { Under = detail }) is not null,
            $"the dial-in row does not show '{pin}'");
    });

    [OfflineFact]
    public void AnEventWithNoMeetingOffersNoJoin() => Case(nameof(AnEventWithNoMeetingOffersNoJoin), (app, flow) =>
    {
        var detail = flow.OpenDetail(Plain);
        Assert.True(app.Agent.Find(new Filter { Id = "EventJoin", Under = detail }) is null,
            "an event with no meeting offers Join");
    });
}
