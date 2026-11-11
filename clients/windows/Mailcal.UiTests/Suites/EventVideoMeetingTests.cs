// Putting a video meeting on an event in the editor, and taking it off (docs/conferencing.md,
// rules 9 to 13), against the harness, whose calendar server writes a meeting made elsewhere into
// the event. Which choices a calendar offers, and what a save sends, are pinned in Mailcal.Tests
// (EventEditorMeetingTests); what this is for is the round trip: a pick the dialog never hands the
// state, a dispatch that drops the change, or a detail that never sees the meeting the save made.
//
// The harness calendar has no meeting service of its own, so its only choice is Jitsi, which is
// offered only once a Jitsi server is set. The cases run in the order written: the first four build
// on one event, and the last of them removes it and the server it set; the last two do the same for
// an event that takes the account's default meeting (rule 10), and the default with it.

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
    private static readonly string DefaultTitle = $"Default meeting {DateTime.Now:yyyyMMdd-HHmmss}";

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

    /// <summary>What the row shows, read off a freshly found box: the form lays itself out again
    /// as the editor finishes opening, and a peer read mid-layout is refused.</summary>
    private static string Selection(App app, Element editor)
    {
        string? selection = null;
        App.Eventually(() =>
        {
            try
            {
                selection = MeetingBox(app, editor) is { } box ? app.Agent.Read(box).Selection : null;
                return selection is not null;
            }
            catch (AgentRefusal)
            {
                return false;
            }
        }, TimeSpan.FromSeconds(5));
        return selection ?? throw new InvalidOperationException("the Video meeting row shows no choice");
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
    private static void Choose(App app, Element editor, string label)
    {
        Pick(app, MeetingBox(app, editor) ?? throw new InvalidOperationException("the editor draws no Video meeting row"), label);
        Assert.Equal(label, Selection(app, editor));
    }

    /// <summary>Clicks <paramref name="label"/> in the open list of <paramref name="box"/>.</summary>
    private static void Pick(App app, Element box, string label)
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
    }

    /// <summary>Waits for the row to show <paramref name="label"/>: a roster change redraws it a
    /// moment after the list changed.</summary>
    private static bool RowComesToShow(App app, Element editor, string label) =>
        App.Eventually(() => Selection(app, editor) == label, TimeSpan.FromSeconds(5));

    /// <summary>Sets the harness account's default meeting in Settings → Videoconferencing, by
    /// clicking as a person does.</summary>
    private static void SetDefault(App app, string label)
    {
        app.CloseDialogs();
        var dialog = new SetupFlow(app).OpenCategory("videoconferencing");
        // The picker is named by the account's address, as is the text beside it.
        var box = app.Agent.Wait(new Filter { Name = Harness.Alice, Type = "ComboBox", Under = dialog }, TimeSpan.FromSeconds(5))
            ?? throw new InvalidOperationException($"Settings → Videoconferencing draws no default meeting for {Harness.Alice}");
        app.Agent.ScrollIntoView(box);
        Pick(app, box, label);
        // The core echoes the choice back as a calendar change; the picker the person used stays
        // where it is, so their focus and a screen reader's place stay on it.
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
        Assert.Equal(label, app.Agent.Read(box).Selection);
        app.CloseDialogs();
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

    private static Filter AddedOnSave(Element editor) => new() { Id = "EventMeetingAddedOnSave", Under = editor };

    private static bool OffersJoin(App app, Element detail) =>
        app.Agent.Wait(new Filter { Id = "EventJoin", Type = "Button", Under = detail }, TimeSpan.FromSeconds(5)) is not null;

    /// <summary>Waits until the event's detail offers Join, or no longer does, as
    /// <paramref name="join"/> says. The agenda redraws as a write settles, so a row found a moment
    /// before can be gone by the press; that is a look to take again, not an answer.</summary>
    private static bool DetailComesToOffer(App app, CalendarFlow flow, bool join, string? title = null) =>
        App.Eventually(() =>
        {
            try
            {
                var offers = OffersJoin(app, flow.OpenDetail(title ?? Title));
                app.CloseDialogs();
                return offers == join;
            }
            catch (AgentRefusal)
            {
                app.CloseDialogs();
                return false;
            }
        }, TimeSpan.FromSeconds(30));

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
        Assert.Equal(none, Selection(app, editor));
        Assert.Equal(new[] { none, Jitsi }, Entries(app, box));
        Assert.True(app.Agent.Find(AddedOnSave(editor)) is null, "with no meeting chosen, nothing is said about a link (rule 10)");
        Choose(app, editor, Jitsi);
        Assert.True(app.Agent.Wait(AddedOnSave(editor), TimeSpan.FromSeconds(5)) is not null,
            "a chosen meeting says its link is added at the save (rule 10)");
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "PrimaryButton", Under = editor })!);
        Assert.True(app.Agent.WaitGone(Filter.ById("EventTitleBox"), TimeSpan.FromSeconds(10)), "Save closes the editor");

        Assert.True(DetailComesToOffer(app, flow, join: true),
            "the saved event's detail offers Join on the Jitsi meeting the editor chose");
        Assert.Equal(Jitsi, app.Agent.Find(new Filter { Id = "EventMeetingService", Under = flow.OpenDetail(Title) })?.Name);
    });

    [HarnessFact]
    public void TheEditorOpensOnTheMeetingAndNoneTakesItOff() => Case(nameof(TheEditorOpensOnTheMeetingAndNoneTakesItOff), (app, flow) =>
    {
        var editor = flow.OpenEditor(Title);
        Assert.True(MeetingBox(app, editor) is not null, "an event with a meeting draws the Video meeting row");
        // Rule 11: the row opens on the meeting the event has, as the choice that made it.
        Assert.Equal(Jitsi, Selection(app, editor));
        Assert.True(app.Agent.Find(AddedOnSave(editor)) is null, "the meeting the event has is not announced as one still to come (rule 10)");
        Choose(app, editor, Catalog.English("event_meeting_none"));
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "PrimaryButton", Under = editor })!);
        Assert.True(app.Agent.WaitGone(Filter.ById("EventTitleBox"), TimeSpan.FromSeconds(10)), "Save closes the editor");

        Assert.True(DetailComesToOffer(app, flow, join: false),
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

    [HarnessFact]
    public void ANewEventTakesTheDefaultWithItsFirstInviteeAndSavesIt() => Case(nameof(ANewEventTakesTheDefaultWithItsFirstInviteeAndSavesIt), (app, flow) =>
    {
        SetServer(app, Server);
        SetDefault(app, Jitsi);
        var editor = flow.OpenNewEvent();
        app.Agent.SetValue(app.Agent.Find(new Filter { Id = "EventTitleBox", Under = editor })!, DefaultTitle);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        var none = Catalog.English("event_meeting_none");
        Assert.Equal(none, Selection(app, editor));
        Assert.True(app.Agent.Find(AddedOnSave(editor)) is null, "with nobody invited, the default is not taken, so nothing is said about a link (rule 10)");

        flow.RevealAtEnd(editor, "EventInviteeField", "the editor lost its invitee field");
        flow.InviteeKeys(Harness.Dave, enter: true);
        Assert.True(RowComesToShow(app, editor, Jitsi), "the first invitee brings the account's default into the row (rule 10)");
        Assert.True(app.Agent.Wait(AddedOnSave(editor), TimeSpan.FromSeconds(5)) is not null, "and says its link is added at the save");

        app.Agent.Invoke(flow.RevealAtEnd(editor, $"EventInviteeRemove:{Harness.Dave}", "Dave is not on the list to remove"));
        Assert.True(RowComesToShow(app, editor, none), "the last invitee leaving takes the default back out (rule 10)");

        flow.RevealAtEnd(editor, "EventInviteeField", "the editor lost its invitee field");
        flow.InviteeKeys(Harness.Dave, enter: true);
        Assert.True(RowComesToShow(app, editor, Jitsi), "an invitee added again brings the default back");
        app.Agent.Invoke(app.Agent.Find(new Filter { Id = "PrimaryButton", Under = editor })!);
        Assert.True(app.Agent.WaitGone(Filter.ById("EventTitleBox"), TimeSpan.FromSeconds(10)), "Save closes the editor");

        Assert.True(DetailComesToOffer(app, flow, join: true, DefaultTitle),
            "the saved event's detail offers Join on the meeting the default made");
        Assert.Equal(Jitsi, app.Agent.Find(new Filter { Id = "EventMeetingService", Under = flow.OpenDetail(DefaultTitle) })?.Name);
        Harness.WaitSchedulingMail(Harness.Dave, "Invitation:", DefaultTitle);
    });

    [HarnessFact]
    public void DeletingTheDefaultsEventAndClearingTheDefaultLeavesTheHarnessAsItWas() => Case(nameof(DeletingTheDefaultsEventAndClearingTheDefaultLeavesTheHarnessAsItWas), (app, flow) =>
    {
        // The default and the server are put back even when the case before never made its event.
        try
        {
            flow.OpenDetail(DefaultTitle);
            app.Agent.Invoke(app.Agent.Find(Filter.ById("SecondaryButton", "Button"))!);
            app.Agent.Invoke(app.Agent.Wait(new Filter { Id = "PrimaryButton", Type = "Button" }, TimeSpan.FromSeconds(10))!);
            Harness.WaitSchedulingMail(Harness.Dave, "Cancelled:", DefaultTitle);
        }
        finally
        {
            Harness.RemoveMail(Harness.Dave, DefaultTitle);
            Harness.RemoveMail(Harness.Alice, DefaultTitle);
            SetDefault(app, Catalog.English("event_meeting_none"));
            SetServer(app, "");
        }
    });
}
