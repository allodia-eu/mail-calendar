// The attendee row's text, tested without a view. Small rules about what the user reads are exactly
// the kind that regress silently: the second line exists to add something the first line does not
// already say, so an unnamed attendee must not get their own address printed twice.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct EventAttendeesTests {
    private func attendee(
        name: String,
        email: String,
        isOrganizer: Bool = false,
        response: ResponseStatus = .accepted
    ) -> EventAttendee {
        EventAttendee(name: name, email: email, isOrganizer: isOrganizer, response: response)
    }

    @Test func aNamedAttendeeGetsTheirAddressOnTheSecondLine() {
        let row = attendee(name: "Anna Jansen", email: "anna@example.com")
        #expect(attendeeSubtitle(row) == "anna@example.com")
    }

    @Test func anUnnamedAttendeeHasNoSecondLineBecauseTheFirstIsTheirAddress() {
        #expect(attendeeSubtitle(attendee(name: "", email: "b@example.com")) == nil)
    }

    @Test func anUnnamedOrganizerStillSaysTheyCalledTheMeeting() {
        let row = attendee(name: "", email: "chair@example.com", isOrganizer: true)
        #expect(attendeeSubtitle(row) == L10n.event_attendee_organizer())
    }

    @Test func aNamedOrganizerShowsBothTheAddressAndTheRole() {
        let row = attendee(name: "Chair Person", email: "chair@example.com", isOrganizer: true)
        #expect(attendeeSubtitle(row) == "chair@example.com · \(L10n.event_attendee_organizer())")
    }

    @Test func everyAnswerHasItsOwnWording() {
        // Five distinct strings: a status that fell through to another's label would be a
        // confidently wrong statement about somebody's answer.
        let labels = [
            attendeeResponseText(.accepted),
            attendeeResponseText(.declined),
            attendeeResponseText(.tentative),
            attendeeResponseText(.delegated),
            attendeeResponseText(.needsAction),
        ]
        #expect(Set(labels).count == 5)
        #expect(!labels.contains(""))
    }

    @Test func theEditorCarriesTheAttendeesThroughSoTheyCanBeShownReadOnly() {
        // The editor prefills from the same detail read; without this the list would be empty in
        // the editor while the detail sheet showed it, on the same event.
        let rows = [attendee(name: "Anna Jansen", email: "anna@example.com")]
        let detail = EventDetail(
            account: "acct",
            key: "/cal/e.ics",
            calendar: "work",
            title: "Standup",
            allDay: false,
            timezone: "Europe/Amsterdam",
            start: "2026-01-05T09:30:00",
            end: "2026-01-05T10:00:00",
            location: nil,
            notes: nil,
            reminderMinutes: nil,
            recurrence: nil,
            repeatSummary: nil,
            isRecurring: false,
            canWrite: true,
            occurrenceStart: "",
            attendees: rows
        )
        let editor = EventEditorState.edit(detail, calendarName: "Work")
        #expect(editor.editing?.attendees.map(\.email) == ["anna@example.com"])
    }
}
