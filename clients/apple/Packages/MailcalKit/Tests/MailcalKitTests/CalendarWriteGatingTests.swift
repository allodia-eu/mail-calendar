// The write-capability gates, pinned as plain values (see CalendarWriteGating.swift).
//
// The policy under test is a cross-client contract: "New event" is DISABLED when no calendar on
// the page can take a write (an empty list, nothing synced yet, counts as no), and a per-event
// delete is HIDDEN when that row's record says its account cannot write. Pure tests, no rendering:
// a rendered header cannot tell you the decision was right, only that it did not crash.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct CalendarWriteGatingTests {

    private func calendarRow(id: String, canWrite: Bool) -> CalendarRow {
        let swatch = Swatch(background: "#16598d", text: "#ffffff", border: "#16598d")
        return CalendarRow(
            account: "acct",
            id: id,
            name: id,
            color: CalendarColor(hex: "#16598d", light: swatch, dark: swatch),
            visible: true,
            canWrite: canWrite,
            isDefault: canWrite
        )
    }

    private func eventRow(canWrite: Bool) -> EventRow {
        EventRow(
            account: "acct",
            key: "evt-1",
            title: "Standup",
            start: "2026-07-16 09:00",
            canWrite: canWrite,
            participation: .accepted
        )
    }

    @Test func newEventNeedsAtLeastOneWritableCalendar() {
        // Nothing synced yet: nowhere a new event could go.
        #expect(!calendarSupportsNewEvent([]))
        // Every calendar read-only: still nowhere.
        #expect(!calendarSupportsNewEvent([calendarRow(id: "work", canWrite: false)]))
        // One writable calendar among read-only ones is enough.
        #expect(
            calendarSupportsNewEvent([
                calendarRow(id: "work", canWrite: false),
                calendarRow(id: "home", canWrite: true),
            ])
        )
    }

    @Test func readOnlyAgendaRowOffersNoDelete() {
        #expect(!eventRow(canWrite: false).offersDelete)
        #expect(eventRow(canWrite: true).offersDelete)
    }
}
