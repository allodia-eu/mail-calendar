// The manual form's port and connection-security rule, as arithmetic. The Apple client's copy of
// the suite every client carries for `ManualServerField` (docs/account-autodetect.md).
//
// What is load-bearing: the picker fills the port until the user takes it over, and a port they
// typed is never overwritten. A server on a non-standard port is the reason the manual form
// exists, so a picker that resets it would defeat the form.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct ManualServerFieldTests {
    @Test func aFreshFieldOffersTheStandardSecurePort() {
        #expect(ManualServerField(.imap).port == "993")
        #expect(ManualServerField(.smtp).port == "465")
        #expect(ManualServerField(.imap).security == .implicitTls)
    }

    @Test func thePortFollowsThePickerWhileItIsStillOurs() {
        var imap = ManualServerField(.imap)
        imap.choose(.startTls)
        #expect(imap.port == "143")
        imap.choose(.implicitTls)
        #expect(imap.port == "993")

        var smtp = ManualServerField(.smtp)
        smtp.choose(.startTls)
        #expect(smtp.port == "587")
    }

    @Test func aPortTypedByHandIsNeverOverwrittenByThePicker() {
        var field = ManualServerField(.imap)
        field.typePort("1143")

        field.choose(.startTls)
        #expect(field.port == "1143")
        field.choose(.implicitTls)
        #expect(field.port == "1143")
        #expect(!field.followsSecurity)
    }

    @Test func clearingThePortHandsItBackToThePicker() {
        var field = ManualServerField(.imap)
        field.typePort("1143")
        field.typePort("   ")

        #expect(field.followsSecurity)
        #expect(field.port == "993")
        field.choose(.startTls)
        #expect(field.port == "143")
    }

    @Test func theDialAddressCarriesTheHostAndThePortTogether() {
        var field = ManualServerField(.imap)
        field.choose(.startTls)
        #expect(field.dial("imap.example.net") == "imap.example.net:143")

        field.typePort("1143")
        #expect(field.dial("127.0.0.1") == "127.0.0.1:1143")
        #expect(field.dial("  127.0.0.1  ") == "127.0.0.1:1143")
    }

    @Test func aPortAlreadyTypedIntoTheHostFieldWins() {
        var field = ManualServerField(.imap)
        field.typePort("1143")
        #expect(field.dial("127.0.0.1:1025") == "127.0.0.1:1025")
    }

    @Test func anEmptyHostStaysEmptySoTheConnectGateStillRefusesIt() {
        #expect(ManualServerField(.imap).dial("   ").isEmpty)
    }

    @Test func aDetectedRouteBringsItsOwnPortAndStopsFollowingThePicker() {
        var field = ManualServerField(.imap)
        field.adoptDetected(host: "imap.example.net:1993", security: .startTls)

        #expect(field.port == "1993")
        #expect(!field.followsSecurity)
        #expect(field.security == .startTls)
    }

    @Test func aDetectedRouteWithoutAPortShowsTheStandardOneForWhatWasDetected() {
        var field = ManualServerField(.imap)
        field.adoptDetected(host: "imap.example.net", security: .startTls)

        #expect(field.port == "143")
        #expect(field.followsSecurity)
    }

    @Test func onlyAnAllDigitTailIsAPort() {
        #expect(ManualServerField.splitHost("imap.example.net").port.isEmpty)
        #expect(ManualServerField.splitHost("imap.example.net:993").host == "imap.example.net")
        #expect(ManualServerField.splitHost("imap.example.net:993").port == "993")
        #expect(ManualServerField.splitHost("host:").port.isEmpty)
        #expect(ManualServerField.splitHost("host:abc").port.isEmpty)
        // Mirrors the core, which splits the same way; a bare IPv6 literal is not a server
        // either side accepts.
        #expect(ManualServerField.splitHost("::1").host == ":")
    }
}
