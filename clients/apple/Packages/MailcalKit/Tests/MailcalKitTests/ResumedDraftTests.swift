// Opening a draft back into its composer, from the Drafts folder or from an edited queued send
// (docs/drafts.md).
//
// The join between the composition and the copy on the server is the core's, and is held by its
// own Rust tests. What is Apple's, and what is covered here, is how the reopened message travels
// to the composer: which composition it saves under, whether opening the same draft twice opens
// the composer twice, and that the editor is seeded with the message's HTML rather than its words.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct ResumedDraftTests {

    private func reopened(
        composition: String = "c-1",
        subject: String = "Half a sentence",
        attachments: [ComposerFileAttachment] = []
    ) -> ComposeRequest {
        ComposeRequest(
            account: "acct-1",
            composition: composition,
            to: "ada@example.test",
            cc: "grace@example.test",
            bcc: "",
            subject: subject,
            bodyHtml: "<p><strong>Half</strong> a sentence</p>",
            bodyText: "Half a sentence",
            attachments: attachments
        )
    }

    @Test func theSameDraftOpenedTwiceIsTwoRequests() {
        // `ComposeContext` is `Identifiable` and iOS presents it with `.fullScreenCover(item:)`,
        // which does nothing when the new item's id equals the one already presented. Two opens of
        // one draft are two composers, so the id is the request's own, not the draft's.
        let first = ResumedDraftRequest(reopened(composition: "c-1"))
        let second = ResumedDraftRequest(reopened(composition: "c-2"))
        #expect(first != second)
        #expect(ComposeContext.resumedDraft(first).id != ComposeContext.resumedDraft(second).id)
    }

    @Test func anEditedQueuedSendOpensOnItsCompositionHoldingItsFiles() {
        // The core has saved the queued send into Drafts under `composition`: a composer on a
        // fresh id would save a second copy beside it, and one missing a file would take that
        // file off the draft on its first save.
        let file = ComposerFileAttachment(
            path: "/tmp/staging/terms.pdf",
            fileName: "terms.pdf",
            mediaType: "application/pdf"
        )
        let request = ResumedDraftRequest(reopened(composition: "c-queued", attachments: [file]))
        #expect(request.composition == "c-queued")
        #expect(request.draft.account == "acct-1")
        #expect(request.draft.attachments.map(\.path) == [file.path])
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

    @Test func aWindowShowingADraftIsNamedForIt() {
        let request = ResumedDraftRequest(reopened(subject: "Terms"))
        #expect(ComposeContext.resumedDraft(request).windowTitle == "Terms")
    }

    @Test func aDraftWithNoSubjectIsNamedForWhatItIs() {
        // The Window menu, Cmd-Tab and Mission Control read this, and an empty string there is a
        // window with no name at all (`docs/reading-window.md`).
        let request = ResumedDraftRequest(reopened(subject: ""))
        #expect(ComposeContext.resumedDraft(request).windowTitle == L10n.compose_title_new())
    }
}
