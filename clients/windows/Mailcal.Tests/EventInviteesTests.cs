// The editor's invitee decisions, tested without a dialog: where the list is offered, what a create
// carries, what an organiser's edit sends, and how the field fills the list. The core's two answers
// are stood in for here, since nothing in this suite loads the cdylib; the real ones are tested in
// `mailcal-bindings` (`meeting_editor_tests.rs`). The Windows twin of Android's EventInviteesTest.
using System;
using System.Collections.Generic;
using System.Linq;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class EventInviteesTests
{
    private static readonly DateTime Now = new(2026, 8, 1, 9, 15, 0, DateTimeKind.Unspecified);
    private const MeetingInviteeRole Required = MeetingInviteeRole.Required;
    private const MeetingInviteeRole Optional = MeetingInviteeRole.Optional;

    // The core's check, in the shape that matters here: an `@`, not already listed, not the organiser.
    private static InviteeCheck Check(string typed, IReadOnlyList<string> invited, string organiser)
    {
        var address = typed.Trim();
        if (!address.Contains('@'))
        {
            return new InviteeCheck.NotAnAddress();
        }
        if (string.Equals(address, organiser, StringComparison.OrdinalIgnoreCase))
        {
            return new InviteeCheck.Organiser();
        }
        if (invited.Any(other => string.Equals(other, address, StringComparison.OrdinalIgnoreCase)))
        {
            return new InviteeCheck.AlreadyInvited();
        }
        return new InviteeCheck.Valid(address);
    }

    // The core's patch over the rows an organiser may edit; null when nothing changed.
    private static MeetingInviteePatch? Patch(
        IReadOnlyList<EventAttendee> attendees, IReadOnlyList<MeetingInvitee> invitees)
    {
        var editable = attendees.Where(row => !row.IsOrganizer && row.Role is not null).ToList();
        var upsert = invitees
            .Where(invitee => !editable.Any(row => row.Email == invitee.Email && row.Role == invitee.Role))
            .ToArray();
        var remove = editable
            .Where(row => !invitees.Any(invitee => invitee.Email == row.Email))
            .Select(row => row.Email)
            .ToArray();
        return upsert.Length == 0 && remove.Length == 0 ? null : new MeetingInviteePatch(upsert, remove);
    }

    private static EventAttendee Attendee(string email, MeetingInviteeRole? role, bool isOrganizer = false) =>
        new(string.Empty, email, isOrganizer, role, ResponseStatus.NeedsAction);

    private static EventAttendee[] Roster() =>
    [
        Attendee("me@example.org", null, isOrganizer: true),
        Attendee("grace@example.org", Required),
        Attendee("chair@example.org", null),
    ];

    private static EventDetail Meeting(
        string occurrence = "", bool canEditInvitees = true, EventAttendee[]? attendees = null) =>
        new(
            "acct", "/cal/m.ics", "work", "Planning", false, "Europe/Amsterdam",
            "2026-08-03T10:00:00", "2026-08-03T11:00:00", null, null, null, null, null, null,
            occurrence.Length > 0, true, canEditInvitees, null, occurrence, attendees ?? Roster());

    private static EventEditorState Creating(bool canInvite)
    {
        var editor = EventEditorState.Create(
            new CalendarChoice("acct", "work", "Work", "me@example.org", canInvite),
            "Europe/Amsterdam",
            Now,
            inviteePatchOf: Patch,
            inviteeCheckOf: Check);
        editor.Title = "Planning";
        return editor;
    }

    private static EventEditorState Editing(EventDetail detail) =>
        EventEditorState.Edit(detail, "Work", inviteePatchOf: Patch, inviteeCheckOf: Check);

    [Fact]
    public void A_create_invites_only_on_a_calendar_whose_server_sends_invitations()
    {
        var on = Creating(canInvite: true);
        Assert.True(on.CanInvite);
        on.Roster.FieldChanged("grace@example.org");
        Assert.True(on.Roster.TakeTyped());
        Assert.Equal([new MeetingInvitee(null, "grace@example.org", Required)], on.CreateArgs().Invitees!);

        var off = Creating(canInvite: false);
        Assert.False(off.CanInvite);
        off.Roster.FieldChanged("grace@example.org");
        off.Roster.TakeTyped();
        Assert.Null(off.CreateArgs().Invitees);
    }

    [Fact]
    public void A_create_with_nobody_on_the_list_is_an_appointment() =>
        Assert.Null(Creating(canInvite: true).CreateArgs().Invitees);

    [Fact]
    public void A_finished_address_moves_out_of_the_field_into_the_list()
    {
        var editor = Creating(canInvite: true);
        editor.Roster.FieldChanged("grace@example.org, ada");
        Assert.Equal(["grace@example.org"], editor.Roster.Invitees.Select(invitee => invitee.Email));
        Assert.Equal("ada", editor.Roster.Field);
    }

    [Fact]
    public void A_refused_address_stays_in_the_field_with_the_reason()
    {
        var editor = Creating(canInvite: true);
        foreach (var (typed, said) in new (string, InviteeCheck)[]
        {
            ("not an address", new InviteeCheck.NotAnAddress()),
            ("ME@example.org", new InviteeCheck.Organiser()),
        })
        {
            editor.Roster.FieldChanged(typed);
            Assert.False(editor.Roster.TakeTyped());
            Assert.Equal(said, editor.Roster.Refusal);
            Assert.Equal(typed, editor.Roster.Field);
        }
        editor.Roster.FieldChanged("grace@example.org");
        Assert.True(editor.Roster.TakeTyped());
        Assert.Null(editor.Roster.Refusal);
        editor.Roster.FieldChanged("GRACE@example.org");
        Assert.False(editor.Roster.TakeTyped());
        Assert.Equal(new InviteeCheck.AlreadyInvited(), editor.Roster.Refusal);
    }

    [Fact]
    public void A_finished_address_that_is_refused_stays_in_the_field_rather_than_vanishing()
    {
        // A comma or an accepted suggestion finishes an address; one the core refuses has to be
        // left where it can be corrected.
        var editor = Creating(canInvite: true);
        editor.Roster.FieldChanged("me@example.org, ");
        Assert.Empty(editor.Roster.Invitees);
        Assert.Equal(new InviteeCheck.Organiser(), editor.Roster.Refusal);
        Assert.Equal("me@example.org", editor.Roster.Field);
    }

    [Fact]
    public void Everything_from_the_first_refused_address_stays_in_the_field()
    {
        // A paste: the address before the refused one is added, and the refused one and all after
        // it stay where they can be corrected, rather than the refused one vanishing.
        var editor = Creating(canInvite: true);
        editor.Roster.FieldChanged("ada@example.org, bob, carol@example.org, dav");
        Assert.Equal(["ada@example.org"], editor.Roster.Invitees.Select(invitee => invitee.Email));
        Assert.Equal(new InviteeCheck.NotAnAddress(), editor.Roster.Refusal);
        Assert.Equal("bob, carol@example.org, dav", editor.Roster.Field);

        // Two refused, nothing typed after: the last goes back into the input, not lost.
        editor.Roster.FieldChanged("bad1, bad2, ");
        Assert.Equal("bad1, bad2", editor.Roster.Field);
    }

    [Fact]
    public void Picking_another_calendar_checks_the_list_against_its_organiser()
    {
        var editor = Creating(canInvite: true);
        editor.Roster.FieldChanged("me@home.example");
        Assert.True(editor.Roster.TakeTyped());

        // The account that owns the next calendar is the one already on the list.
        editor.Calendar = new CalendarChoice("other", "home", "Home", "me@home.example", CanInvite: true);
        Assert.False(editor.Roster.Recheck());
        Assert.Equal(new InviteeCheck.Organiser(), editor.Roster.Refusal);
        Assert.False(editor.Roster.TakeTyped());

        editor.Roster.Remove(0);
        Assert.Null(editor.Roster.Refusal);
        Assert.True(editor.Roster.TakeTyped());
    }

    [Fact]
    public void A_save_asks_the_core_for_the_change_once_until_the_list_changes()
    {
        var asked = 0;
        var editor = EventEditorState.Edit(
            Meeting(occurrence: "2026-08-03T10:00:00"),
            "Work",
            inviteePatchOf: (attendees, invitees) => { asked++; return Patch(attendees, invitees); },
            inviteeCheckOf: Check);
        editor.Roster.Remove(0);
        _ = editor.AsksAboutTheSeries;
        _ = editor.UpdateArgs(thisOccurrenceOnly: false);
        Assert.Equal(1, asked);

        editor.Roster.FieldChanged("ada@example.org");
        Assert.True(editor.Roster.TakeTyped());
        Assert.Equal(["ada@example.org"], editor.UpdateArgs(thisOccurrenceOnly: false).Invitees!.Upsert.Select(i => i.Email));
        Assert.Equal(2, asked);
    }

    [Fact]
    public void An_organiser_opens_the_list_with_the_rows_they_may_edit_and_sends_what_changed()
    {
        var editor = Editing(Meeting());
        Assert.True(editor.CanInvite);
        Assert.Equal([new MeetingInvitee(null, "grace@example.org", Required)], editor.Roster.Invitees);
        Assert.Null(editor.UpdateArgs(thisOccurrenceOnly: false).Invitees);

        // The chair is on the meeting already, though no control here may change their row.
        editor.Roster.FieldChanged("chair@example.org");
        Assert.False(editor.Roster.TakeTyped());

        editor.Roster.SetOptional(0, true);
        editor.Roster.FieldChanged("ada@example.org");
        editor.Roster.TakeTyped();
        var change = editor.UpdateArgs(thisOccurrenceOnly: false).Invitees!;
        Assert.Equal(
            [
                new MeetingInvitee(null, "grace@example.org", Optional),
                new MeetingInvitee(null, "ada@example.org", Required),
            ],
            change.Upsert);
        Assert.Empty(change.Remove);

        editor.Roster.Remove(0);
        Assert.Equal(["grace@example.org"], editor.UpdateArgs(thisOccurrenceOnly: false).Invitees!.Remove);
    }

    [Fact]
    public void Somebody_elses_meeting_offers_no_list_and_sends_no_roster()
    {
        var editor = Editing(Meeting(canEditInvitees: false));
        Assert.False(editor.CanInvite);
        Assert.Empty(editor.Roster.Invitees);
        Assert.Null(editor.UpdateArgs(thisOccurrenceOnly: false).Invitees);
    }

    [Fact]
    public void A_changed_roster_settles_the_scope_question_and_never_travels_with_one_occurrence()
    {
        var editor = Editing(Meeting(occurrence: "2026-08-03T10:00:00"));
        Assert.True(editor.AsksAboutTheSeries);
        editor.Roster.Remove(0);
        Assert.False(editor.AsksAboutTheSeries);
        Assert.Equal(["grace@example.org"], editor.UpdateArgs(thisOccurrenceOnly: false).Invitees!.Remove);
        Assert.Null(editor.UpdateArgs(thisOccurrenceOnly: true).Invitees);
    }

    [Fact]
    public void An_existing_meeting_keeps_its_last_invitee_and_a_create_does_not_have_to()
    {
        // The chair is still on the meeting, so Grace may go.
        Assert.False(Editing(Meeting()).Roster.KeepsLastInvitee);

        var withoutChair = Editing(Meeting(attendees: Roster()[..2]));
        Assert.True(withoutChair.Roster.KeepsLastInvitee);
        withoutChair.Roster.FieldChanged("ada@example.org");
        withoutChair.Roster.TakeTyped();
        Assert.False(withoutChair.Roster.KeepsLastInvitee);

        // A create may become an appointment.
        var create = Creating(canInvite: true);
        create.Roster.FieldChanged("ada@example.org");
        create.Roster.TakeTyped();
        Assert.False(create.Roster.KeepsLastInvitee);
    }

    [Fact]
    public void A_create_organises_as_the_account_of_the_calendar_picked()
    {
        var editor = Creating(canInvite: true);
        editor.Calendar = new CalendarChoice("other", "home", "Home", "me@home.example", CanInvite: true);
        editor.Roster.FieldChanged("me@home.example");
        Assert.False(editor.Roster.TakeTyped());
        Assert.Equal(new InviteeCheck.Organiser(), editor.Roster.Refusal);

        editor.Calendar = new CalendarChoice("other", "home", "Home", "me@home.example", CanInvite: false);
        Assert.False(editor.CanInvite);
    }
}
