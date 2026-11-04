// Putting a video meeting on an event in the editor, and taking it off (docs/conferencing.md,
// rules 9 to 13), against the harness, whose calendar server writes a meeting made elsewhere into
// the event. Which choices a calendar offers, and what a save sends, are pinned in Mailcal.Tests
// (EventEditorMeetingTests); what this is for is the round trip: a pick the dialog never hands the
// state, a dispatch that drops the change, or a detail that never sees the meeting the save made.
//
// The harness calendar has no meeting service of its own, so its only choice is Jitsi, which is
// offered only once a Jitsi server is set. The cases build on one event and run in the order
// written; the last removes it and the server it set.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class EventVideoMeetingTests(HarnessApp harness)
{
    private const string Jitsi = "Jitsi Meet";
    private const string Server = "https://meet.example.org";
    private static readonly string Title = $"Video meeting {DateTime.Now:yyyyMMdd-HHmmss}";

    private void Case(string name, Action<App, CalendarFlow> body) =>
        harness.Case(name, app => body(app, new CalendarFlow(app)), app => app.CloseDialogs());

    private static Element? MeetingBox(App app, Element editor)
    {
        var box = app.Agent.Find(new Filter { Id = "EventMeetingBox", Under = editor });
        if (box is not null)
        {
            app.Agent.ScrollIntoView(box);
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(200));
        }
        return box;
    }

    /// <summary>The entries the row offers, read off the open list.</summary>
    private static List<string> Entries(App app, Element box)
    {
        app.Agent.Expand(box);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        var items = app.Agent.FindAll(new Filter { Type = "ListItem", Under = box }).Select(i => i.Name).Distinct().ToList();
        app.Agent.Collapse(box);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(200));
        return items;
    }

    /// <summary>Chooses <paramref name="label"/> in the row. Neither the combo's own items nor the
    /// open list's take a selection through their peer, so the choice is a click, as a person
    /// makes it.</summary>
    private static void Choose(App app, Element box, string label)
    {
        app.Agent.Expand(box);
        var named = Filter.Named(label, "ListItem");
        Element? item = null;
        if (!App.Eventually(() =>
            {
                var closed = app.Agent.FindAll(named with { Under = box }).Select(e => e.Handle).ToHashSet();
                return (item = app.Agent.FindAll(named).FirstOrDefault(e => !closed.Contains(e.Handle))) is not null;
            }, TimeSpan.FromSeconds(3)))
        {
            throw new InvalidOperationException($"the Video meeting row offers no '{label}'");
        }
        Input.Click(app.Steady(item!));
        App.Eventually(() => app.Agent.FindAll(named).Count <= app.Agent.FindAll(named with { Under = box }).Count, TimeSpan.FromSeconds(3));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(200));
        Assert.Equal(label, app.Agent.Read(box).Selection);
    }

    /// <summary>Sets the Jitsi server in Settings → Videoconferencing, committing it by moving the
    /// keyboard off the field.</summary>
    private static void SetServer(App app, string server)
    {
        // One dialog at a time: Settings opens only once whatever was up has finished closing.
        app.CloseDialogs();
        var settings = new SetupFlow(app);
        var dialog = settings.OpenCategory("videoconferencing");
        var box = app.Agent.Find(new Filter { Id = "JitsiServerBox", Under = dialog })
            ?? throw new InvalidOperationException("Settings → Videoconferencing draws no Jitsi server field");
        app.Agent.Focus(box);
        app.Agent.SetValue(box, server);
        app.Agent.Focus(app.Agent.Find(new Filter { Id = "SettingsCategory_videoconferencing", Under = dialog })!);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
        app.CloseDialogs();
    }

    private static bool OffersJoin(App app, Element detail) =>
        app.Agent.Wait(new Filter { Id = "EventJoin", Type = "Button", Under = detail }, TimeSpan.FromSeconds(5)) is not null;

    [HarnessFact]
    public void WithNoJitsiServerTheHarnessCalendarOffersNoMeeting() => Case(nameof(WithNoJitsiServerTheHarnessCalendarOffersNoMeeting), (app, flow) =>
    {
        SetServer(app, "");
        Assert.True(MeetingBox(app, flow.OpenNewEvent()) is null,
            "a calendar with no meeting service of its own and no Jitsi server set has nothing to choose, so no row is drawn (rule 9)");
    });

    [HarnessFact]
    public void ANewEventGetsTheJitsiMeetingItsEditorChose() => Case(nameof(ANewEventGetsTheJitsiMeetingItsEditorChose), (app, flow) =>
    {
        SetServer(app, Server);
        var editor = flow.OpenNewEvent();
        app.Agent.SetValue(app.Agent.Find(new Filter { Id = "EventTitleBox", Under = editor })!, Title);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        var box = MeetingBox(app, editor)
            ?? throw new InvalidOperationException("with a Jitsi server set, the harness calendar offers a Video meeting row (rule 9)");
        var none = Catalog.English("event_meeting_none");
        Assert.Equal(none, app.Agent.Read(box).Selection);
        Assert.Equal(new[] { none, Jitsi }, Entries(app, box));
        Choose(app, box, Jitsi);
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "PrimaryButton", Under = editor })!);
        Assert.True(app.Agent.WaitGone(Filter.ById("EventTitleBox"), TimeSpan.FromSeconds(10)), "Save closes the editor");

        Assert.True(App.Eventually(() => OffersJoin(app, flow.OpenDetail(Title)), TimeSpan.FromSeconds(30)),
            "the saved event's detail offers Join on the Jitsi meeting the editor chose");
    });

    [HarnessFact]
    public void TheEditorOpensOnTheMeetingAndNoneTakesItOff() => Case(nameof(TheEditorOpensOnTheMeetingAndNoneTakesItOff), (app, flow) =>
    {
        var editor = flow.OpenEditor(Title);
        var box = MeetingBox(app, editor) ?? throw new InvalidOperationException("an event with a meeting draws the Video meeting row");
        // Rule 11: the row opens on the meeting the event has, whatever the core names it.
        var none = Catalog.English("event_meeting_none");
        Assert.NotEqual(none, app.Agent.Read(box).Selection);
        Choose(app, box, none);
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "PrimaryButton", Under = editor })!);
        Assert.True(app.Agent.WaitGone(Filter.ById("EventTitleBox"), TimeSpan.FromSeconds(10)), "Save closes the editor");

        Assert.True(App.Eventually(() =>
            {
                var gone = !OffersJoin(app, flow.OpenDetail(Title));
                app.CloseDialogs();
                return gone;
            }, TimeSpan.FromSeconds(30)),
            "choosing None takes the meeting off, so the detail offers no Join (rule 11)");
    });

    [HarnessFact]
    public void DeletingTheEventAndClearingTheServerLeavesTheHarnessAsItWas() => Case(nameof(DeletingTheEventAndClearingTheServerLeavesTheHarnessAsItWas), (app, flow) =>
    {
        flow.OpenDetail(Title);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SecondaryButton", "Button"))!);
        app.Agent.Invoke(app.Agent.Wait(new Filter { Id = "PrimaryButton", Type = "Button" }, TimeSpan.FromSeconds(10))!);
        SetServer(app, "");
    });
}
