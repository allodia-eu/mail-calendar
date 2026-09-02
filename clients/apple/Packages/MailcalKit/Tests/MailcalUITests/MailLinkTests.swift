// Opening a `mailto:` link in the composer (docs/os-integration.md).
//
// The URI *parsing* is the shared core's and is held by its own Rust tests. What is Apple's, and
// what is covered here, is the identity of the request: a link that arrives twice must open the
// composer twice. That fails silently, the second tap simply appears to do nothing, which is why
// it earns a test rather than a comment.
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
        // `ComposeContext` is `Identifiable` and iOS presents it with `.fullScreenCover(item:)`,
        // which does nothing when the new item's id equals the one already presented. Without a
        // per-request id, tapping the same link again would compare equal and never reopen.
        let first = MailLinkRequest(prefill: prefill(to: "ada@example.test"))
        let second = MailLinkRequest(prefill: prefill(to: "ada@example.test"))

        #expect(first != second)
        #expect(ComposeContext.mailLink(first).id != ComposeContext.mailLink(second).id)
    }

    @Test func aMailLinkIsItsOwnComposeContext() {
        // It must not collide with the blank composer's id: `.new` is a constant, so a link
        // opening while a new message is up would otherwise read as the same presentation.
        let request = MailLinkRequest(prefill: prefill(to: "ada@example.test"))
        #expect(ComposeContext.mailLink(request).id != ComposeContext.new.id)
        #expect(ComposeContext.mailLink(request).id.hasPrefix("mailLink:"))
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
