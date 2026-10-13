// Opening a draft back into its composer, from the Drafts folder or from an edited queued send
// (docs/drafts.md).
//
// The join between the composition and the copy on the server is the core's, and is held by its
// own Rust tests. What is Apple's, and what is covered here, is how the reopened message travels
// to the composer: what it opens with, which files are its baseline, and that the editor is
// seeded with the message's HTML rather than its words.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct ResumedDraftTests {

    @Test func anEditedQueuedSendOpensThroughItsStoredBody() {
        // The core has saved the queued send into Drafts under the opening's composition. It opens
        // through its own HTML, with its text beside it, holding every file it was queued with, so
        // its baseline is all of them and closing it untouched is not "written".
        let file = ComposerFileAttachment(
            path: "/tmp/staging/terms.pdf",
            fileName: "terms.pdf",
            mediaType: "application/pdf"
        )
        let opening = composeOpening(
            composition: "c-queued", bodyHtml: "<p>Terms</p>", bodyText: "Terms",
            attachments: [file], seedsSignature: false
        )
        let context = composeContext(opening)
        #expect(context.mode == .new)
        #expect(context.plainBody == "Terms")
        #expect(context.initialError == nil)
        #expect(
            ComposeOpeningSeed.baselineAttachmentCount(
                opening: opening.attachments, added: context.addedAttachments
            ) == 1
        )
    }

    @Test func aForwardWhoseFilesCouldNotBeStagedOpensSayingSo() {
        let opening = ComposeOpening(
            composition: "c-1", kind: .forward, account: "acct-1", to: "", cc: "", bcc: "",
            subject: "Fwd: Terms", bodyHtml: "", bodyText: "", quote: nil, attachments: [],
            attachmentsFailed: true, seedsSignature: true
        )
        #expect(composeContext(opening).initialError == L10n.compose_forward_attachments_failed())
    }

    @Test func aReopenedMessageSeedsItsHTMLAsData() throws {
        // A body seeded as words alone opened without its formatting, pictures, quote and
        // signature, and the composer's next save took them off the draft.
        let script = try #require(
            RichComposerEditor.bodySeedScript(html: "<p>Hi</p>\")</script>", text: "Hi")
        )
        #expect(script.hasPrefix("window.setComposerBody("))
        let argument = String(script.dropFirst("window.setComposerBody(".count).dropLast())
        let json = try #require(
            JSONSerialization.jsonObject(with: Data(argument.utf8), options: .fragmentsAllowed)
                as? String
        )
        let seed = try #require(
            JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: String]
        )
        #expect(seed["html"] == "<p>Hi</p>\")</script>")
        #expect(seed["text"] == "Hi")
    }

    @Test func aBodyWithNoHTMLIsSeededAsText() {
        #expect(RichComposerEditor.bodySeedScript(html: nil, text: "Hi") == "window.setPlainText(\"Hi\")")
        #expect(RichComposerEditor.bodySeedScript(html: "", text: "") == nil)
    }
}
