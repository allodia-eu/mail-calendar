// The calendar write-status badge's mapping from the core's `CalendarWriteStatus`.
//
// The mapping is the whole point that can go wrong client-side: the core already decides the status;
// this side only turns it into what the header shows. A pure test pins it without rendering (a
// rendered SwiftUI badge cannot tell you the mapping is right, only that it did not crash).

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct CalendarWriteIndicatorTests {

    @Test func everyStatusMapsToAnIndicator() {
        #expect(CalendarWriteIndicator.of(.idle) == .hidden)
        #expect(CalendarWriteIndicator.of(.saving) == .spinner)
        #expect(CalendarWriteIndicator.of(.saved) == .saved)
        #expect(CalendarWriteIndicator.of(.failed) == .warning)
    }

    @Test func onlyTheWarningOffersARetry() {
        // The retry is a refresh, and it only makes sense on the unconfirmed state, offering it on a
        // spinner or a check would invite the user to "retry" a write that is fine.
        #expect(CalendarWriteIndicator.warning.offersRetry)
        #expect(!CalendarWriteIndicator.spinner.offersRetry)
        #expect(!CalendarWriteIndicator.saved.offersRetry)
        #expect(!CalendarWriteIndicator.hidden.offersRetry)
    }
}
