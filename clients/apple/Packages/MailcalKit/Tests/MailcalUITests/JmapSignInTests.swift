// The JMAP sign-in button's probe gate. The button itself is only shown once the core answers
// "yes, this server advertises OAuth", but that answer costs a blocking network round trip, so
// this decides which typed addresses are even worth asking about. Pure logic, no SwiftUI.

import Foundation
import Testing

@testable import MailcalUI

struct JmapSignInTests {
    @Test func aCompleteAddressIsProbed() {
        #expect(JmapOAuthProbe.looksLikeAddress("alice@fastmail.com"))
        #expect(JmapOAuthProbe.looksLikeAddress("alice@mail.example.co.uk"))
    }

    @Test func aHalfTypedAddressIsNotProbed() {
        // Every one of these is a state the field passes through while the user types, and each
        // would otherwise fire a blocking probe at a domain they never meant.
        #expect(!JmapOAuthProbe.looksLikeAddress(""))
        #expect(!JmapOAuthProbe.looksLikeAddress("alice"))
        #expect(!JmapOAuthProbe.looksLikeAddress("alice@"))
        #expect(!JmapOAuthProbe.looksLikeAddress("alice@fastmail"))
        #expect(!JmapOAuthProbe.looksLikeAddress("alice@fastmail."))
        #expect(!JmapOAuthProbe.looksLikeAddress("@fastmail.com"))
    }

    @Test func aMalformedAddressIsNotProbed() {
        #expect(!JmapOAuthProbe.looksLikeAddress("alice@@fastmail.com"))
        #expect(!JmapOAuthProbe.looksLikeAddress("alice@.com"))
    }
}
