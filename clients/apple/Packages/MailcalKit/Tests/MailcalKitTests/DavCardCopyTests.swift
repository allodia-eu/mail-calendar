// What the calendar-only card says (docs/onboarding.md, "After the connect: linking another
// account"): beside a JMAP account its servers add to that account, so the card does not claim
// the address has no mail server.

import MailcalBindings
import Testing

@testable import MailcalUI

private func card(beside: Bool) -> DetectedSetup {
    DetectedSetup(
        recommendation: .jmap(
            email: "eva@fastmail.example", serverUrl: "https://api.fastmail.example/",
            isTrusted: true, source: "srv"),
        calendarAndContacts: true, caldavUrl: "https://dav.fastmail.example/caldav/",
        carddavUrl: nil, choices: [], nextcloudSignIn: false, besideAccount: beside)
}

@Suite struct DavCardCopyTests {
    @Test func theCardBesideAJmapAccountSaysWhatItAdds() {
        #expect(DavCardCopy.note(card(beside: true)) == L10n.setup_detect_dav_beside_note())
        #expect(DavCardCopy.note(card(beside: true)) != L10n.setup_detect_dav_note())
    }

    @Test func aCardDetectionFoundSaysThereIsNoMailServer() {
        #expect(DavCardCopy.note(card(beside: false)) == L10n.setup_detect_dav_note())
    }
}
