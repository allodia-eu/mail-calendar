// Opening a draft from the Drafts folder back into its composer (docs/drafts.md).
//
// The join between the composition and the copy on the server is the core's, and is held by its
// own Rust tests. What is Apple's, and what is covered here, is how the resumed draft travels to
// the composer: which composition it saves under, and whether opening the same draft twice opens
// the composer twice.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct ResumedDraftTests {

    private func resume(subject: String = "Half a sentence") -> DraftResume {
        DraftResume(
            account: "acct-1",
            to: "ada@example.test",
            cc: "",
            bcc: "",
            subject: subject,
            bodyText: "Half a sentence",
            attachments: []
        )
    }

    @Test func theSameDraftOpenedTwiceIsTwoRequests() {
        // `ComposeContext` is `Identifiable` and iOS presents it with `.fullScreenCover(item:)`,
        // which does nothing when the new item's id equals the one already presented. Two opens of
        // one draft are two composers, so the id is the request's own, not the draft's.
        let first = ResumedDraftRequest(composition: "c-1", draft: resume())
        let second = ResumedDraftRequest(composition: "c-2", draft: resume())
        #expect(first != second)
        #expect(ComposeContext.resumedDraft(first).id != ComposeContext.resumedDraft(second).id)
    }

    @Test func theCompositionTravelsWithTheDraft() {
        // The core has already joined this id to the copy on the server: a composer that took a
        // fresh one would store a second draft beside the one it is showing, and neither would
        // supersede the other.
        let request = ResumedDraftRequest(composition: "c-1", draft: resume())
        #expect(request.composition == "c-1")
    }

    @Test func aWindowShowingADraftIsNamedForIt() {
        let request = ResumedDraftRequest(composition: "c-1", draft: resume(subject: "Terms"))
        #expect(ComposeContext.resumedDraft(request).windowTitle == "Terms")
    }

    @Test func aDraftWithNoSubjectIsNamedForWhatItIs() {
        // The Window menu, Cmd-Tab and Mission Control read this, and an empty string there is a
        // window with no name at all (`docs/reading-window.md`).
        let request = ResumedDraftRequest(composition: "c-1", draft: resume(subject: ""))
        #expect(ComposeContext.resumedDraft(request).windowTitle == L10n.compose_title_new())
    }
}
