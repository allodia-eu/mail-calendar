// The editor's invitee decisions, tested without a view: where the list is offered, what a create
// carries, what an organiser's edit sends, and how the field fills the list. The core's two answers
// (`checkMeetingInvitee`, `meetingInviteePatch`) are the real ones, linked from the xcframework.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct EventInviteesTests {
    private var now: Date {
        Calendar.current.date(from: DateComponents(year: 2026, month: 8, day: 1, hour: 9, minute: 15))!
    }

    private func attendee(_ email: String, _ role: MeetingInviteeRole?, isOrganizer: Bool = false) -> EventAttendee {
        EventAttendee(name: "", email: email, isOrganizer: isOrganizer, role: role, response: .needsAction)
    }

    private func meeting(
        occurrence: String = "",
        canEditInvitees: Bool = true,
        withChair: Bool = true
    ) -> EventDetail {
        var attendees = [
            attendee("me@example.org", nil, isOrganizer: true),
            attendee("grace@example.org", .required),
        ]
        if withChair { attendees.append(attendee("chair@example.org", nil)) }
        return EventDetail(
            account: "acct",
            key: "/cal/m.ics",
            calendar: "work",
            title: "Planning",
            allDay: false,
            timezone: "Europe/Amsterdam",
            start: "2026-08-03T10:00:00",
            end: "2026-08-03T11:00:00",
            location: nil,
            notes: nil,
            reminderMinutes: nil,
            recurrence: nil,
            repeatSummary: nil,
            repeatDraft: nil,
            isRecurring: !occurrence.isEmpty,
            canWrite: true,
            canEditInvitees: canEditInvitees,
            myAnswer: nil,
            occurrenceStart: occurrence,
            attendees: attendees
        )
    }

    private func creating(canInvite: Bool) -> EventEditorState {
        var editor = EventEditorState.create(
            default: CalendarChoice(
                account: "acct", id: "work", name: "Work", address: "me@example.org", canInvite: canInvite
            ),
            zone: "Europe/Amsterdam",
            now: now
        )
        editor.title = "Planning"
        return editor
    }

    private func editing(_ detail: EventDetail) -> EventEditorState {
        EventEditorState.edit(detail, calendarName: "Work")
    }

    @Test func aCreateInvitesOnlyOnACalendarWhoseServerSendsInvitations() {
        var on = creating(canInvite: true)
        #expect(on.canInvite)
        on.inviteeFieldChanged("grace@example.org")
        #expect(take(&on))
        #expect(on.createArgs().invitees == [MeetingInvitee(name: nil, email: "grace@example.org", role: .required)])

        var off = creating(canInvite: false)
        #expect(!off.canInvite)
        off.inviteeFieldChanged("grace@example.org")
        _ = off.takeTypedInvitee()
        #expect(off.createArgs().invitees == nil)
    }

    @Test func aCreateWithNobodyOnTheListIsAnAppointment() {
        #expect(creating(canInvite: true).createArgs().invitees == nil)
    }

    @Test func aFinishedAddressMovesOutOfTheFieldIntoTheList() {
        var editor = creating(canInvite: true)
        // What the field writes back when a suggestion is accepted, or a comma typed.
        editor.inviteeFieldChanged(acceptRecipientSuggestion("gra", "grace@example.org"))
        editor.inviteeFieldChanged("ada")
        #expect(editor.roster.invitees.map(\.email) == ["grace@example.org"])
        #expect(editor.roster.field == "ada")
    }

    @Test func aRefusedAddressStaysInTheFieldWithTheReason() {
        var editor = creating(canInvite: true)
        for (typed, said) in [("not an address", InviteeCheck.notAnAddress), ("ME@example.org", .organiser)] {
            editor.inviteeFieldChanged(typed)
            #expect(!take(&editor))
            #expect(editor.roster.refusal == said)
            #expect(editor.roster.field == typed)
        }
        editor.inviteeFieldChanged("grace@example.org")
        #expect(take(&editor))
        #expect(editor.roster.refusal == nil)
        #expect(editor.roster.field.isEmpty)

        // Finished with a comma, a refused address is put back rather than dropped.
        editor.inviteeFieldChanged("GRACE@example.org, ")
        #expect(editor.roster.refusal == .alreadyInvited)
        #expect(editor.roster.field == "GRACE@example.org")
        #expect(editor.roster.invitees.count == 1)
    }

    @Test func theOrganiserFollowsTheCalendarPickedOnACreate() {
        var editor = creating(canInvite: true)
        editor.calendar = CalendarChoice(
            account: "other", id: "home", name: "Home", address: "home@example.org", canInvite: true
        )
        editor.inviteeFieldChanged("me@example.org")
        #expect(take(&editor))
        editor.inviteeFieldChanged("home@example.org")
        #expect(!take(&editor))
        #expect(editor.roster.refusal == .organiser)
    }

    @Test func anOrganiserOpensTheListWithTheRowsTheyMayEditAndSendsWhatChanged() {
        var editor = editing(meeting())
        #expect(editor.canInvite)
        #expect(editor.roster.invitees == [MeetingInvitee(name: nil, email: "grace@example.org", role: .required)])
        #expect(editor.updateArgs(thisOccurrenceOnly: false).invitees == nil)

        // The chair is on the meeting already, though no control here may change their row.
        editor.inviteeFieldChanged("chair@example.org")
        #expect(!take(&editor))
        #expect(editor.roster.refusal == .alreadyInvited)

        editor.roster.setOptional(true, at: 0)
        editor.inviteeFieldChanged("ada@example.org")
        #expect(take(&editor))
        #expect(
            editor.updateArgs(thisOccurrenceOnly: false).invitees
                == MeetingInviteePatch(
                    upsert: [
                        MeetingInvitee(name: nil, email: "grace@example.org", role: .optional),
                        MeetingInvitee(name: nil, email: "ada@example.org", role: .required),
                    ],
                    remove: []
                )
        )
        editor.roster.remove(at: 0)
        #expect(editor.updateArgs(thisOccurrenceOnly: false).invitees?.remove == ["grace@example.org"])
    }

    @Test func somebodyElsesMeetingOffersNoListAndSendsNoRoster() {
        let editor = editing(meeting(canEditInvitees: false))
        #expect(!editor.canInvite)
        #expect(editor.roster.invitees.isEmpty)
        #expect(editor.updateArgs(thisOccurrenceOnly: false).invitees == nil)
    }

    @Test func aChangedRosterSettlesTheScopeQuestionAndNeverTravelsWithOneOccurrence() {
        var editor = editing(meeting(occurrence: "2026-08-03T10:00:00"))
        #expect(editor.asksAboutTheSeries)
        editor.roster.remove(at: 0)
        #expect(!editor.asksAboutTheSeries)
        #expect(editor.updateArgs(thisOccurrenceOnly: false).invitees?.remove == ["grace@example.org"])
        #expect(editor.updateArgs(thisOccurrenceOnly: true).invitees == nil)
    }

    @Test func anExistingMeetingKeepsItsLastInviteeAndACreateDoesNotHaveTo() {
        // The chair is still on the meeting, so Grace may go.
        #expect(!editing(meeting()).roster.keepsLastInvitee)

        var withoutChair = editing(meeting(withChair: false))
        #expect(withoutChair.roster.keepsLastInvitee)
        withoutChair.inviteeFieldChanged("ada@example.org")
        _ = withoutChair.takeTypedInvitee()
        #expect(!withoutChair.roster.keepsLastInvitee)

        // A create may become an appointment.
        var create = creating(canInvite: true)
        create.inviteeFieldChanged("ada@example.org")
        _ = create.takeTypedInvitee()
        #expect(!create.roster.keepsLastInvitee)
    }

    @Test func everyRefusalHasItsOwnWording() {
        let said = [InviteeCheck.notAnAddress, .alreadyInvited, .organiser].compactMap(inviteeRefusal)
        #expect(Set(said).count == 3)
        #expect(inviteeRefusal(.valid(address: "a@example.org")) == nil)
    }
}

/// Save's first step, outside `#expect`, which cannot hold a mutating call.
private func take(_ editor: inout EventEditorState) -> Bool {
    editor.takeTypedInvitee()
}
