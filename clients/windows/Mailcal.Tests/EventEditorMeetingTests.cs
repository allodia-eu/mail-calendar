// The editor's Video meeting row (docs/conferencing.md, rules 9 to 11), tested without a dialog:
// what it offers, what it opens showing, and what a save sends. The Windows twin of Apple's
// EventEditorMeetingTests and Android's EventEditorMeetingTest.
using System;
using System.Linq;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class EventEditorMeetingTests
{
    private static readonly DateTime Now = new(2026, 8, 1, 9, 15, 0, DateTimeKind.Unspecified);
    private static readonly MeetingOption Teams = new("server:teams", "Microsoft Teams");
    private static readonly MeetingOption Jitsi = new("jitsi", "Jitsi Meet");

    private static CalendarChoice Calendar(MeetingOption[] choices, string id = "work") =>
        new("acct", id, id, MeetingChoices: choices);

    private static EventEditorState Editing(
        EditorMeeting? meeting, MeetingOption[] choices, string occurrence = "") =>
        EventEditorState.Edit(
            new EventDetail(
                "acct", "/cal/e.ics", "work", "Standup", false, "Europe/Amsterdam",
                "2026-08-03T09:00:00", "2026-08-03T09:30:00", null, null, null, null, null, null,
                occurrence.Length > 0, true, false, null, occurrence, [], null, null, [], meeting),
            "Work",
            choices);

    private static string[] Labels(EventEditorState editor) =>
        [.. editor.MeetingOptions.Select(option => option.Label("None", "Video meeting", (service, account) => $"{service} ({account})"))];

    [Fact]
    public void A_new_event_starts_with_no_meeting_and_offers_the_calendars_choices()
    {
        var editor = EventEditorState.Create(Calendar([Teams, Jitsi]), "UTC", Now);
        editor.Title = "Review";
        Assert.True(editor.ShowsMeetingRow);
        Assert.Equal(
            new MeetingSelection[]
            {
                new MeetingSelection.None(),
                new MeetingSelection.Choice("server:teams"),
                new MeetingSelection.Choice("jitsi"),
            },
            editor.MeetingOptions.Select(option => option.Selection));
        Assert.Null(editor.CreateArgs().Meeting);

        editor.Meeting = new MeetingSelection.Choice("jitsi");
        Assert.Equal(new MeetingPick.Set("jitsi"), editor.CreateArgs().Meeting);
    }

    private static CalendarChoice Inviting(MeetingOption[] choices, string? defaultMeeting, string id = "work") =>
        new("acct", id, id, CanInvite: true, MeetingChoices: choices, DefaultMeeting: defaultMeeting);

    // A new event whose invitee field takes any address, so nothing here loads the cdylib.
    private static EventEditorState Creating(CalendarChoice calendar) =>
        EventEditorState.Create(
            calendar, "UTC", Now, inviteeCheckOf: (typed, _, _) => new InviteeCheck.Valid(typed.Trim()));

    private static void Invite(EventEditorState editor)
    {
        editor.Roster.FieldChanged("grace@test.local");
        Assert.True(editor.Roster.TakeTyped());
    }

    [Fact]
    public void A_new_event_takes_its_calendars_default_once_it_has_an_invitee()
    {
        var editor = Creating(Inviting([Teams, Jitsi], "jitsi"));
        editor.Title = "Review";
        // An event with nobody invited makes no meeting.
        Assert.Equal(new MeetingSelection.None(), editor.EffectiveMeeting);
        Assert.Null(editor.CreateArgs().Meeting);

        Invite(editor);
        Assert.Equal(new MeetingSelection.Choice("jitsi"), editor.EffectiveMeeting);
        Assert.True(editor.AnnouncesMeeting);
        Assert.Equal(new MeetingPick.Set("jitsi"), editor.CreateArgs().Meeting);
    }

    [Fact]
    public void A_default_needs_a_calendar_that_can_invite()
    {
        var editor = Creating(Inviting([Teams, Jitsi], "jitsi") with { CanInvite = false });
        Invite(editor);
        Assert.Equal(new MeetingSelection.None(), editor.EffectiveMeeting);
    }

    [Fact]
    public void A_pick_in_the_row_outranks_the_default()
    {
        var editor = Creating(Inviting([Teams, Jitsi], "jitsi"));
        editor.Title = "Review";
        editor.Meeting = new MeetingSelection.None();
        Invite(editor);
        Assert.Equal(new MeetingSelection.None(), editor.EffectiveMeeting);
        Assert.Null(editor.CreateArgs().Meeting);

        editor.Meeting = new MeetingSelection.Choice("server:teams");
        Assert.Equal(new MeetingPick.Set("server:teams"), editor.CreateArgs().Meeting);
    }

    [Fact]
    public void An_untouched_row_follows_the_new_calendars_default()
    {
        var editor = Creating(Inviting([Teams, Jitsi], "jitsi"));
        Invite(editor);
        editor.Calendar = Inviting([Teams], "server:teams", id: "office");
        Assert.Equal(new MeetingSelection.Choice("server:teams"), editor.EffectiveMeeting);
        editor.Calendar = Inviting([Teams], null, id: "plain");
        Assert.Equal(new MeetingSelection.None(), editor.EffectiveMeeting);
    }

    [Fact]
    public void An_edit_never_takes_a_default()
    {
        var editor = Editing(null, [Teams, Jitsi]);
        Assert.Equal(new MeetingSelection.None(), editor.EffectiveMeeting);
        Assert.Null(editor.UpdateArgs(thisOccurrenceOnly: false).Meeting);
    }

    [Fact]
    public void A_calendar_with_no_choices_draws_no_row()
    {
        Assert.False(EventEditorState.Create(Calendar([]), "UTC", Now).ShowsMeetingRow);
        // A calendar row read before the core offered any carries no list at all.
        Assert.False(EventEditorState.Create(new CalendarChoice("acct", "work", "Work"), "UTC", Now).ShowsMeetingRow);
    }

    [Fact]
    public void A_choice_the_new_calendar_lacks_goes_back_to_none()
    {
        var editor = EventEditorState.Create(Calendar([Teams, Jitsi]), "UTC", Now);
        editor.Title = "Review";
        editor.Meeting = new MeetingSelection.Choice("server:teams");
        editor.Calendar = Calendar([Jitsi], id: "home");
        Assert.Equal(new MeetingSelection.None(), editor.EffectiveMeeting);
        Assert.Null(editor.CreateArgs().Meeting);
    }

    [Fact]
    public void An_edit_opens_on_the_meeting_and_leaves_it_alone_until_the_row_changes()
    {
        var editor = Editing(new EditorMeeting("Microsoft Teams", "server:teams"), [Teams, Jitsi]);
        Assert.Equal(new MeetingSelection.Choice("server:teams"), editor.Meeting);
        Assert.Null(editor.UpdateArgs(thisOccurrenceOnly: false).Meeting);

        editor.Meeting = new MeetingSelection.Choice("jitsi");
        Assert.Equal(new MeetingPick.Set("jitsi"), editor.UpdateArgs(thisOccurrenceOnly: false).Meeting);

        editor.Meeting = new MeetingSelection.None();
        Assert.Equal(new MeetingPick.Remove(), editor.UpdateArgs(thisOccurrenceOnly: false).Meeting);
        // A meeting belongs to the series.
        Assert.Null(editor.UpdateArgs(thisOccurrenceOnly: true).Meeting);
    }

    [Fact]
    public void A_meeting_no_choice_makes_is_shown_by_name_and_can_only_be_taken_off()
    {
        var editor = Editing(new EditorMeeting("Zoom", null), [Jitsi]);
        Assert.Equal(new MeetingSelection.Current(), editor.Meeting);
        Assert.Equal(["None", "Zoom", "Jitsi Meet"], Labels(editor));
        Assert.Null(editor.UpdateArgs(thisOccurrenceOnly: false).Meeting);

        editor.Meeting = new MeetingSelection.None();
        Assert.Equal(new MeetingPick.Remove(), editor.UpdateArgs(thisOccurrenceOnly: false).Meeting);
    }

    [Fact]
    public void A_meeting_on_a_calendar_that_offers_none_can_still_be_taken_off()
    {
        var editor = Editing(new EditorMeeting(null, "jitsi"), []);
        Assert.True(editor.ShowsMeetingRow);
        Assert.Equal(["None", "Video meeting"], Labels(editor));
        Assert.Equal(new MeetingSelection.Choice("jitsi"), editor.EffectiveMeeting);
    }

    [Fact]
    public void A_changed_meeting_settles_which_occurrences_the_save_meant()
    {
        // Opened on one occurrence, the save would ask; a meeting is the series', so choosing one
        // answers the question the way a changed repeat does.
        var editor = Editing(null, [Jitsi], occurrence: "2026-08-03T09:00:00");
        Assert.True(editor.AsksAboutTheSeries);
        editor.Meeting = new MeetingSelection.Choice("jitsi");
        Assert.False(editor.AsksAboutTheSeries);
        Assert.Equal(new MeetingPick.Set("jitsi"), editor.UpdateArgs(thisOccurrenceOnly: false).Meeting);
    }

    [Fact]
    public void A_meeting_made_through_an_account_names_the_account()
    {
        var work = new MeetingOption("account:teams:work", "Microsoft Teams", "alice@work.example");
        var other = new MeetingOption("account:teams:other", "Microsoft Teams", "alice@other.example");
        var editor = EventEditorState.Create(Calendar([work, other, Jitsi]), "UTC", Now);
        Assert.Equal(
            ["None", "Microsoft Teams (alice@work.example)", "Microsoft Teams (alice@other.example)", "Jitsi Meet"],
            Labels(editor));
    }

    [Fact]
    public void Only_a_choice_the_event_does_not_have_yet_announces_the_link_at_the_save()
    {
        var created = EventEditorState.Create(Calendar([Teams, Jitsi]), "UTC", Now);
        Assert.False(created.AnnouncesMeeting);
        created.Meeting = new MeetingSelection.Choice("jitsi");
        Assert.True(created.AnnouncesMeeting);
        created.Meeting = new MeetingSelection.None();
        Assert.False(created.AnnouncesMeeting);

        var opened = Editing(new EditorMeeting("Microsoft Teams", "server:teams"), [Teams, Jitsi]);
        Assert.False(opened.AnnouncesMeeting);
        opened.Meeting = new MeetingSelection.Choice("jitsi");
        Assert.True(opened.AnnouncesMeeting);
        opened.Meeting = new MeetingSelection.Choice("server:teams");
        Assert.False(opened.AnnouncesMeeting);

        // A meeting no choice makes is the event's own, so it has nothing to announce.
        Assert.False(Editing(new EditorMeeting("Zoom", null), [Jitsi]).AnnouncesMeeting);
    }

    [Fact]
    public void A_pick_carries_the_words_it_is_sent_with()
    {
        var words = new MeetingWords("Video meeting", "Join", "Meeting ID", "Passcode", "Join by phone", "Conference ID");
        Assert.Equal(new MeetingChange.Set("jitsi", words), new MeetingPick.Set("jitsi").Change(words));
        Assert.IsType<MeetingChange.Remove>(new MeetingPick.Remove().Change(words));
    }
}
