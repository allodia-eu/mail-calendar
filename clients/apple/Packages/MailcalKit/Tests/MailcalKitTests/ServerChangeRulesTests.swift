// The dialog that asks about a calendar or address book server on another site
// (docs/account-autodetect.md rule 14): which title, body and caution it reads for a calendar, an
// address book or both, and that the sites are set in bold as plain text.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

private func change(calendar: Bool, contacts: Bool) -> ServerChange {
    ServerChange(
        previousSite: "example.org", proposedSite: "example.net",
        calendar: calendar, contacts: contacts)
}

@Suite struct ServerChangeLabelsTests {
    @Test func aCalendarAloneReadsAsTheCalendar() {
        let moved = change(calendar: true, contacts: false)
        #expect(ServerChangeLabels.title(moved) == L10n.server_changed_title_calendar())
        #expect(
            ServerChangeLabels.body(moved, address: "a@example.org")
                == L10n.server_changed_body_calendar(address: "a@example.org"))
        #expect(ServerChangeLabels.unsure(moved) == L10n.server_changed_unsure_calendar())
    }

    @Test func anAddressBookAloneReadsAsTheAddressBook() {
        let moved = change(calendar: false, contacts: true)
        #expect(ServerChangeLabels.title(moved) == L10n.server_changed_title_contacts())
        #expect(
            ServerChangeLabels.body(moved, address: "a@example.org")
                == L10n.server_changed_body_contacts(address: "a@example.org"))
        #expect(ServerChangeLabels.unsure(moved) == L10n.server_changed_unsure_contacts())
    }

    @Test func bothReadAsBoth() {
        let moved = change(calendar: true, contacts: true)
        #expect(ServerChangeLabels.title(moved) == L10n.server_changed_title_both())
        #expect(
            ServerChangeLabels.body(moved, address: "a@example.org")
                == L10n.server_changed_body_both(address: "a@example.org"))
        #expect(ServerChangeLabels.unsure(moved) == L10n.server_changed_unsure_both())
    }

    @Test func theSitesAreTheCatalogLinesWithTheSiteInBold() {
        let moved = change(calendar: true, contacts: false)
        let before = ServerChangeLabels.before(moved)
        let now = ServerChangeLabels.now(moved)
        #expect(String(before.characters) == L10n.server_changed_before(site: "example.org"))
        #expect(String(now.characters) == L10n.server_changed_now(site: "example.net"))
        #expect(bold(before) == ["example.org"])
        #expect(bold(now) == ["example.net"])
    }

    @Test func aSiteIsNeverReadAsMarkup() {
        let line = ServerChangeLabels.emphasised("Now: **x**.test", "**x**.test")
        #expect(String(line.characters) == "Now: **x**.test")
        #expect(bold(line) == ["**x**.test"])
    }

    private func bold(_ text: AttributedString) -> [String] {
        text.runs.filter { $0.inlinePresentationIntent == .stronglyEmphasized }
            .map { String(text[$0.range].characters) }
    }
}
