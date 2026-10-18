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
        #expect(CalendarWriteIndicator.of(.notSaved) == .notSaved)
        #expect(CalendarWriteIndicator.of(.unconfirmed) == .unconfirmed)
    }

    @Test func aChangeThatWasNotSavedIsNotCalledUnconfirmed() {
        #expect(CalendarWriteIndicator.notSaved.warning == L10n.calendar_save_failed())
        #expect(CalendarWriteIndicator.unconfirmed.warning == L10n.calendar_save_unconfirmed())
        #expect(CalendarWriteIndicator.hidden.warning == nil)
        #expect(CalendarWriteIndicator.spinner.warning == nil)
        #expect(CalendarWriteIndicator.saved.warning == nil)
    }

    @Test func onlyAWarningOffersARetry() {
        // The retry is a refresh, and it only makes sense on a warning, offering it on a spinner or
        // a check would invite the user to "retry" a write that is fine.
        #expect(CalendarWriteIndicator.notSaved.offersRetry)
        #expect(CalendarWriteIndicator.unconfirmed.offersRetry)
        #expect(!CalendarWriteIndicator.spinner.offersRetry)
        #expect(!CalendarWriteIndicator.saved.offersRetry)
        #expect(!CalendarWriteIndicator.hidden.offersRetry)
    }
}

/// The invitation card's line about its answer, and the contacts list's line about a save.
@Suite struct WriteStatusLineTests {
    @Test func everyCalendarStatusReadsAsTheInvitationLine() {
        #expect(invitationWriteLine(.idle) == nil)
        #expect(invitationWriteLine(.saving) == nil)
        #expect(invitationWriteLine(.saved) == nil)
        #expect(invitationWriteLine(.notSaved) == L10n.invitation_failed())
        #expect(invitationWriteLine(.unconfirmed) == L10n.invitation_unconfirmed())
    }

    @Test func everyContactStatusReadsAsTheListLine() {
        #expect(MailboxModel.contactWriteLine(.idle) == nil)
        #expect(MailboxModel.contactWriteLine(.saving) == L10n.contacts_saving())
        #expect(MailboxModel.contactWriteLine(.saved) == L10n.contacts_saved())
        #expect(MailboxModel.contactWriteLine(.notSaved) == L10n.contacts_save_failed())
        #expect(MailboxModel.contactWriteLine(.unconfirmed) == L10n.contacts_save_unconfirmed())
        #expect(MailboxModel.contactWriteLine(.invalid) == nil)
    }
}
