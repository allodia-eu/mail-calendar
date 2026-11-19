// The delete gate, pinned as a plain value (see CalendarWriteGating.swift): a per-event delete is
// HIDDEN when that row's record says it cannot write.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct CalendarWriteGatingTests {

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

    @Test func readOnlyAgendaRowOffersNoDelete() {
        #expect(!eventRow(canWrite: false).offersDelete)
        #expect(eventRow(canWrite: true).offersDelete)
    }
}
