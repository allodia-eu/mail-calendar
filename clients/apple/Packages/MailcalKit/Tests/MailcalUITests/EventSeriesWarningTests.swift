// The two series questions an editor puts: which occurrences a save meant, and, when the answer
// is all of them, what that costs the occurrences the user singled out.
//
// The verdict itself is the core's, and no harness can raise it (docs/calendar.md → "Known
// gaps"), so what is pinned here is the pair either side of it: which sentence each verdict gets,
// and which occurrences each answer sends.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct EventSeriesWarningTests {
    private func detail(occurrence: String) -> EventDetail {
        EventDetail(
            account: "acct",
            key: "/cal/e.ics",
            calendar: "work",
            title: "Standup",
            allDay: false,
            timezone: "Europe/Amsterdam",
            start: "2026-08-26T09:00:00",
            end: "2026-08-26T09:15:00",
            location: nil,
            notes: nil,
            reminderMinutes: nil,
            recurrence: nil,
            repeatSummary: nil,
            repeatDraft: nil,
            isRecurring: true,
            canWrite: true,
            occurrenceStart: occurrence,
            attendees: []
        )
    }

    @Test func nothingToSayIsNoSentence() {
        #expect(seriesWarningText(nil) == nil)
    }

    @Test func eachVerdictGetsItsOwnSentence() {
        let texts = [
            seriesWarningText(.occurrencesReset),
            seriesWarningText(.renamesSpread),
            seriesWarningText(.occurrencesResetAndRenamesSpread),
        ]
        #expect(texts.allSatisfy { $0?.isEmpty == false })
        // Three distinct verdicts, three distinct sentences: a catalog key wired twice would say
        // the wrong thing about the user's calendar and nothing on screen would tell them apart.
        #expect(Set(texts.compactMap { $0 }).count == 3)
    }

    @Test func anEditorOpenedOnOneOccurrenceAsksWhichOnesTheSaveMeant() {
        let editor = EventEditorState.edit(
            detail(occurrence: "2026-09-09T09:00:00"), calendarName: "Work"
        )
        #expect(editor.asksAboutTheSeries)
    }

    @Test func anEditorOpenedOnTheSeriesAsksNothing() {
        // An agenda row, and every one-off event: there is no single day to name, so the only
        // thing the save can mean is the series.
        let editor = EventEditorState.edit(detail(occurrence: ""), calendarName: "Work")
        #expect(!editor.asksAboutTheSeries)
    }

    @Test func thisEventSendsTheOccurrenceAndAllEventsWithholdsIt() {
        let editor = EventEditorState.edit(
            detail(occurrence: "2026-09-09T09:00:00"), calendarName: "Work"
        )
        // The whole scope question comes down to this one field, so it is asserted on both
        // answers: withholding it on *This event* would rewrite every occurrence, and sending it
        // on *All events* would split an override instead of moving the series.
        #expect(
            editor.updateArgs(thisOccurrenceOnly: true).occurrence == "2026-09-09T09:00:00"
        )
        #expect(editor.updateArgs(thisOccurrenceOnly: false).occurrence == nil)
    }

    @Test func anEditorOnTheSeriesNamesNoOccurrenceEitherWay() {
        // Nothing to name, so even the answer that would send one cannot: a client that sent an
        // empty token would have the core refuse a write that should have gone through.
        let editor = EventEditorState.edit(detail(occurrence: ""), calendarName: "Work")
        #expect(editor.updateArgs(thisOccurrenceOnly: true).occurrence == nil)
        #expect(editor.updateArgs(thisOccurrenceOnly: false).occurrence == nil)
    }

    @Test func bothAnswersStillCarryBothEdges() {
        // An occurrence's own times are not the series', so a single-occurrence edit that named
        // neither edge would move it onto the master's clock.
        let editor = EventEditorState.edit(
            detail(occurrence: "2026-09-09T09:00:00"), calendarName: "Work"
        )
        let args = editor.updateArgs(thisOccurrenceOnly: true)
        #expect(args.start?.isEmpty == false)
        #expect(args.end?.isEmpty == false)
    }
}
