// The event editor's decisions, tested without a view: the two payload shapes (a zoned wall-clock
// create, an event-zone edit), the all-day inclusive↔exclusive conversion that bites both ways, the
// fields frozen on edit, and validity. Pure values, the wall clocks round-trip through the device
// calendar, so the assertions hold whatever zone the test machine is in.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct EventEditorStateTests {
    // A fixed "now", built from components so the wall clock is stable on any machine.
    private var now: Date {
        Calendar.current.date(from: DateComponents(year: 2026, month: 8, day: 1, hour: 9, minute: 15))!
    }

    private func detail(
        allDay: Bool,
        timezone: String,
        start: String,
        end: String,
        isRecurring: Bool = false,
        reminderMinutes: Int32? = nil,
        recurrence: EventRecurrence? = nil,
        attendees: [EventAttendee] = []
    ) -> EventDetail {
        EventDetail(
            account: "acct",
            key: "/cal/e.ics",
            calendar: "work",
            title: "Standup",
            allDay: allDay,
            timezone: timezone,
            start: start,
            end: end,
            location: "Room 2",
            notes: "bring the roadmap",
            reminderMinutes: reminderMinutes,
            recurrence: recurrence,
            repeatSummary: nil,
            repeatDraft: nil,
            isRecurring: isRecurring,
            canWrite: true,
            occurrenceStart: "",
            attendees: attendees
        )
    }

    @Test func createdTimedEventIsAWallClockInTheDeviceZone() {
        var editor = EventEditorState.create(
            default: CalendarChoice(account: "acct", id: "work", name: "Work"),
            zone: "Europe/Amsterdam",
            now: now
        )
        editor.title = "Lunch"
        editor.location = "Room 6" // a create can set a location now (engine PR)
        let args = editor.createArgs()
        #expect(args.title == "Lunch")
        #expect(args.start == "2026-08-01T10:00:00") // next whole hour after 09:15
        #expect(args.end == "2026-08-01T11:00:00")
        #expect(args.timezone == "Europe/Amsterdam")
        #expect(args.account == "acct")
        #expect(args.calendar == "work")
        #expect(args.allDay == false)
        #expect(args.notes == nil)
        #expect(args.location == "Room 6")
    }

    @Test func createdEventWithNoLocationSendsNone() {
        // Empty stays absent, the core turns a nil into no LOCATION line at all.
        var editor = EventEditorState.create(
            default: CalendarChoice(account: "acct", id: "work", name: "Work"),
            zone: "Europe/Amsterdam",
            now: now
        )
        editor.title = "Lunch"
        #expect(editor.createArgs().location == nil)
    }

    @Test func createdAllDayEventSendsAnExclusiveEndAndNoZone() {
        var editor = EventEditorState.create(
            default: CalendarChoice(account: "acct", id: "work", name: "Work"),
            zone: "Europe/Amsterdam",
            now: now
        )
        editor.title = "Holiday"
        editor.allDay = true // one day: start and end both fall on 2026-08-01
        let args = editor.createArgs()
        #expect(args.allDay == true)
        #expect(args.start == "2026-08-01")
        #expect(args.end == "2026-08-02") // exclusive
        #expect(args.timezone == nil)
    }

    @Test func editingPrefillsTheOwnWallClockAndUpdatesIt() {
        var editor = EventEditorState.edit(
            detail(allDay: false, timezone: "Europe/Amsterdam", start: "2026-01-05T09:30:00", end: "2026-01-05T10:00:00"),
            calendarName: "Work"
        )
        #expect(editor.isEditing)
        #expect(editor.title == "Standup")
        #expect(editor.location == "Room 2")

        editor.title = "Standup (kort)"
        let args = editor.updateArgs(thisOccurrenceOnly: false)
        #expect(args.account == "acct")
        #expect(args.key == "/cal/e.ics")
        #expect(args.title == "Standup (kort)")
        #expect(args.start == "2026-01-05T09:30:00")
        #expect(args.end == "2026-01-05T10:00:00")
        #expect(args.location == "Room 2")
        #expect(args.occurrence == nil) // v1 edits the whole series
    }

    @Test func editingAnAllDayEventShowsTheInclusiveDayAndSavesTheExclusiveOne() {
        // The detail's end is exclusive (04-02 for a one-day event on the 1st). The editor must show
        // the 1st and save the 2nd again, an off-by-one here grows a one-day event to two.
        let editor = EventEditorState.edit(
            detail(allDay: true, timezone: "", start: "2026-04-01", end: "2026-04-02"),
            calendarName: "Work"
        )
        #expect(editor.allDay)
        let args = editor.updateArgs(thisOccurrenceOnly: false)
        #expect(args.start == "2026-04-01")
        #expect(args.end == "2026-04-02")
    }

    @Test func allDayAndCalendarAreFrozenOnEditButFreeOnCreate() {
        #expect(EventEditorState.create(default: nil, zone: "Europe/Amsterdam", now: now).canEditForm)
        #expect(
            EventEditorState.edit(
                detail(allDay: false, timezone: "Europe/Amsterdam", start: "2026-01-05T09:30:00", end: "2026-01-05T10:00:00"),
                calendarName: "Work"
            ).canEditForm == false
        )
    }

    @Test func anEditorIsInvalidWithoutATitleOrAPositiveInterval() {
        var editor = EventEditorState.create(
            default: CalendarChoice(account: "acct", id: "work", name: "Work"),
            zone: "Europe/Amsterdam",
            now: now
        )
        #expect(editor.isValid == false) // blank title
        editor.title = "X"
        #expect(editor.isValid)
        editor.end = editor.start
        #expect(editor.isValid == false) // end must be after start
    }

    @Test func aRefAsksAboutTheSeriesExactlyWhenItNamesAnOccurrence() {
        // The guard on the delete path: the detail is reached from a grid block, a month chip or
        // an agenda row, and only the first two name the day the user was looking at. An agenda
        // row is the series, so a delete from there is a series delete and asks nothing.
        let fromTheGrid = EventRefID(
            account: "acct", key: "/cal/e.ics", occurrence: "2026-08-04T09:00:00"
        )
        let fromTheAgenda = EventRefID(account: "acct", key: "/cal/e.ics", occurrence: "")
        #expect(fromTheGrid.asksAboutTheSeries)
        #expect(fromTheAgenda.asksAboutTheSeries == false)
    }

    @Test func remindersBucketIntoTheCoarsestExactUnit() {
        #expect(reminderBucket(nil) == .none)
        #expect(reminderBucket(0) == .atStart)
        #expect(reminderBucket(15) == .minutes(15))
        #expect(reminderBucket(120) == .hours(2))
        #expect(reminderBucket(1440) == .days(1))
    }
}
