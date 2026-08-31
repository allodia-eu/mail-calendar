// The one piece of signature logic the CLIENT owns: which of an account's two slots a composer
// opened in a given mode reads from (docs/signatures.md). Everything else, the library, the
// assignment, the resolution, the sanitising, the `data:`→`cid:` rewrite, lives in Rust and is
// tested there.
//
// It is pinned here rather than through the UI because the rule is a *grouping decision*, not a
// layout: a reply, a reply-all and a forward deliberately share one slot, and the way that breaks
// is silently, a forward quietly taking the new-message signature reads as "signatures work"
// until someone notices the wrong one went out.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct SignatureSlotTests {
    @Test func aNewMessageReadsTheNewMessageSlot() {
        #expect(signatureSlot(for: .new) == .newMessage)
    }

    @Test func replyReplyAllAndForwardAllShareTheReplyForwardSlot() {
        // Outlook's grouping: all three continue an existing message. Splitting them produces a
        // setting nobody sets, and a forward landing on the *new-message* slot is the specific
        // regression this guards.
        #expect(signatureSlot(for: .reply) == .replyForward)
        #expect(signatureSlot(for: .replyAll) == .replyForward)
        #expect(signatureSlot(for: .forward) == .replyForward)
    }
}
