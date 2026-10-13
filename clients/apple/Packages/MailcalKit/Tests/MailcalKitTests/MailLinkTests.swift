// Opening a `mailto:` link in the composer (docs/os-integration.md).
//
// The URI *parsing* is the shared core's and is held by its own Rust tests. What is Apple's, and
// what is covered here, is the identity of the request: a link that arrives twice is two requests,
// and keeps every field the core decoded.
//
// The other client-side rule Gate 12 binds, that a pre-filled Cc/Bcc opens the collapsed row, is
// held by `RecipientTokenTests.aPreFilledCcOrBccOpensTheCollapsedRow` and is shared with every
// other route that pre-fills a composer.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct MailLinkTests {

    private func prefill(to: String = "", subject: String = "") -> MailtoPrefill {
        MailtoPrefill(to: to, cc: "", bcc: "", subject: subject, body: "")
    }

    @Test func theSameLinkTwiceIsTwoRequests() {
        // Two taps on one link are two requests, never one value seen twice.
        let first = MailLinkRequest(prefill: prefill(to: "ada@example.test"))
        let second = MailLinkRequest(prefill: prefill(to: "ada@example.test"))

        #expect(first != second)
    }

    @Test func aRequestKeepsEveryFieldTheCoreDecoded() {
        // The client adds an id and nothing else: it may not re-derive, drop or repair a field,
        // because the allowlist that produced them is the core's (Gate 12).
        let decoded = MailtoPrefill(
            to: "ada@example.test",
            cc: "carol@example.test",
            bcc: "snoop@evil.test",
            subject: "Lunch",
            body: "One\nTwo"
        )
        let request = MailLinkRequest(prefill: decoded)

        #expect(request.prefill.to == "ada@example.test")
        #expect(request.prefill.cc == "carol@example.test")
        #expect(request.prefill.bcc == "snoop@evil.test")
        #expect(request.prefill.subject == "Lunch")
        #expect(request.prefill.body == "One\nTwo")
    }

    @Test func aBccFromALinkOpensTheCollapsedRow() {
        // The security-relevant half, asserted on the shape a mail link actually produces: RFC
        // 6068 lets a link set a Bcc, and a recipient the sender cannot see is one they cannot
        // remove before pressing Send.
        let request = MailLinkRequest(
            prefill: MailtoPrefill(to: "ada@example.test", cc: "", bcc: "snoop@evil.test",
                                   subject: "", body: "")
        )
        #expect(revealsCcBcc(cc: request.prefill.cc, bcc: request.prefill.bcc))
    }
}
