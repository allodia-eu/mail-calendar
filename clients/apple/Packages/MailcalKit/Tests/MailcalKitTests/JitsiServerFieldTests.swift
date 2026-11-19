// What Settings → Videoconferencing's Jitsi field shows from the core's page (see
// JitsiServerField.swift). Whether an address is stored, tidied or refused is the core's rule and is
// tested there.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct JitsiServerFieldTests {

    private func page(stored: String? = nil, refused: String? = nil) -> VideoconferencingPage {
        VideoconferencingPage(jitsiServer: stored, jitsiRefused: refused, defaults: [])
    }

    @Test func theRefusalShowsWhileTheFieldHoldsTheRefusedText() {
        let refused = page(refused: "http://meet.test.local")
        #expect(jitsiRefusalShown(text: "http://meet.test.local", page: refused))
        #expect(!jitsiRefusalShown(text: "http://meet.test.loca", page: refused))
    }

    @Test func nothingRefusedShowsNoRefusal() {
        #expect(!jitsiRefusalShown(text: "", page: page()))
        #expect(!jitsiRefusalShown(text: "https://meet.test.local", page: page(stored: "https://meet.test.local")))
    }

    @Test func aSendTheCoreTookShowsTheStoredForm() {
        let stored = page(stored: "https://meet.test.local")
        #expect(jitsiFieldText(afterSending: " https://meet.test.local/ ", page: stored) == "https://meet.test.local")
    }

    @Test func aBlankSendShowsAnEmptyField() {
        #expect(jitsiFieldText(afterSending: "  ", page: page()) == "")
    }

    @Test func aRefusedSendKeepsTheText() {
        let refused = page(stored: "https://meet.test.local", refused: "http://other.test.local")
        #expect(jitsiFieldText(afterSending: "http://other.test.local", page: refused) == "http://other.test.local")
    }
}
