// When a share may leave the box for the composer (docs/os-integration.md, "One share is on screen
// at a time").
//
// Taking a share spends it, so a share opened at the wrong moment is a share lost: the composer it
// replaces closes, and one nobody has written in closes without a draft, taking its files with it.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct ShareGateTests {

    @Test func aLookTakesAShareUnlessOneIsOnScreenOrWaitingForAnAccount() {
        #expect(ShareGate.mayTake(with: .empty, sharePending: false))
        #expect(!ShareGate.mayTake(with: .share, sharePending: false))
        #expect(!ShareGate.mayTake(with: .empty, sharePending: true))
    }

    @Test func aShareJustMadeOpensOverAnotherComposer() {
        // Sharing is the user's own action, made now: the composer it replaces keeps what was
        // written by saving it to Drafts on the way out.
        #expect(ShareGate.mayTake(with: .other, sharePending: false))
    }

    @Test func closingTheLastComposerLooksInTheBox() {
        #expect(ShareGate.looksAfter(from: .share, to: .empty))
        #expect(ShareGate.looksAfter(from: .other, to: .empty))
    }

    @Test func aComposerReplacingAShareLeavesTheNextOneWaiting() {
        // The share's composer is gone, but a reply or a mail link the user opened took its place,
        // and the share waiting in the box must not replace that in turn.
        #expect(!ShareGate.looksAfter(from: .share, to: .other))
    }

    @Test func openingAComposerLooksNowhere() {
        #expect(!ShareGate.looksAfter(from: .empty, to: .other))
        #expect(!ShareGate.looksAfter(from: .empty, to: .share))
        #expect(!ShareGate.looksAfter(from: .other, to: .share))
    }

    @Test func theSlotIsReadFromTheComposeContext() {
        #expect(ShareGate.Composer(nil) == .empty)
        #expect(ShareGate.Composer(.new) == .other)
        let prefill = SharePrefill(
            to: "", cc: "", bcc: "", subject: "", body: "",
            attachments: [], rejected: [], isEmpty: false
        )
        #expect(ShareGate.Composer(.share(ShareOpenRequest(prefill: prefill))) == .share)
    }
}
