// The editor's Video meeting row (docs/conferencing.md, rules 9 to 11), tested without a view: what
// it offers, what it opens showing, and what a save sends.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct EventEditorMeetingTests {
    private let teams = MeetingOption(id: "server:teams", name: "Microsoft Teams")
    private let jitsi = MeetingOption(id: "jitsi", name: "Jitsi Meet")

    private var now: Date {
        Calendar.current.date(from: DateComponents(year: 2026, month: 8, day: 1, hour: 9, minute: 15))!
    }

    private func calendar(_ choices: [MeetingOption], id: String = "work") -> CalendarChoice {
        CalendarChoice(account: "acct", id: id, name: id, meetingChoices: choices)
    }

    private func editing(_ meeting: EditorMeeting?, choices: [MeetingOption]) -> EventEditorState {
        let detail = EventDetail(
            account: "acct",
            key: "/cal/e.ics",
            calendar: "work",
            title: "Standup",
            allDay: false,
            timezone: "Europe/Amsterdam",
            start: "2026-08-03T09:00:00",
            end: "2026-08-03T09:30:00",
            location: nil,
            notes: nil,
            reminderMinutes: nil,
            recurrence: nil,
            repeatSummary: nil,
            repeatDraft: nil,
            isRecurring: false,
            canWrite: true,
            canEditInvitees: false,
            myAnswer: nil,
            occurrenceStart: "",
            attendees: [],
            editorMeeting: meeting
        )
        return EventEditorState.edit(detail, calendarName: "Work", meetingChoices: choices)
    }

    @Test func aNewEventStartsWithNoMeetingAndOffersTheCalendarsChoices() {
        var editor = EventEditorState.create(default: calendar([teams, jitsi]), zone: "UTC", now: now)
        editor.title = "Review"
        #expect(editor.showsMeetingRow)
        #expect(editor.meetingOptions.map(\.label) == [L10n.event_meeting_none(), "Microsoft Teams", "Jitsi Meet"])
        #expect(editor.createArgs().meeting == nil)

        editor.meeting = .choice("jitsi")
        #expect(editor.createArgs().meeting == .set(choice: "jitsi", words: .catalog))
    }

    @Test func aCalendarWithNoChoicesDrawsNoRow() {
        let editor = EventEditorState.create(default: calendar([]), zone: "UTC", now: now)
        #expect(!editor.showsMeetingRow)
    }

    @Test func aChoiceTheNewCalendarLacksGoesBackToNone() {
        var editor = EventEditorState.create(default: calendar([teams, jitsi]), zone: "UTC", now: now)
        editor.title = "Review"
        editor.meeting = .choice("server:teams")
        editor.calendar = calendar([jitsi], id: "home")
        #expect(editor.effectiveMeeting == .none)
        #expect(editor.createArgs().meeting == nil)
    }

    @Test func anEditOpensOnTheMeetingAndLeavesItAloneUntilTheRowChanges() {
        var editor = editing(EditorMeeting(name: "Microsoft Teams", choice: "server:teams"), choices: [teams, jitsi])
        #expect(editor.meeting == .choice("server:teams"))
        #expect(editor.updateArgs(thisOccurrenceOnly: false).meeting == nil)

        editor.meeting = .choice("jitsi")
        #expect(editor.updateArgs(thisOccurrenceOnly: false).meeting == .set(choice: "jitsi", words: .catalog))

        editor.meeting = .none
        #expect(editor.updateArgs(thisOccurrenceOnly: false).meeting == .remove)
        // A meeting belongs to the series.
        #expect(editor.updateArgs(thisOccurrenceOnly: true).meeting == nil)
    }

    @Test func aMeetingNoChoiceMakesIsShownByNameAndCanOnlyBeTakenOff() {
        var editor = editing(EditorMeeting(name: "Zoom", choice: nil), choices: [jitsi])
        #expect(editor.meeting == .current)
        #expect(editor.meetingOptions.map(\.label) == [L10n.event_meeting_none(), "Zoom", "Jitsi Meet"])
        #expect(editor.updateArgs(thisOccurrenceOnly: false).meeting == nil)

        editor.meeting = .none
        #expect(editor.updateArgs(thisOccurrenceOnly: false).meeting == .remove)
    }

    @Test func aMeetingOnACalendarThatOffersNoneCanStillBeTakenOff() {
        let editor = editing(EditorMeeting(name: nil, choice: "jitsi"), choices: [])
        #expect(editor.showsMeetingRow)
        #expect(editor.meetingOptions.map(\.label) == [L10n.event_meeting_none(), L10n.event_video_meeting()])
        #expect(editor.effectiveMeeting == .choice("jitsi"))
    }
}
