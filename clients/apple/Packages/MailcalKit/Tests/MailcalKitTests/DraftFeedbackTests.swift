// Feedback on a drafted reply (docs/ai.md, "Feedback"): thumbs up kept at once with nothing of the
// draft, thumbs down through the form, the email and the draft never included unless ticked, and a
// later draft starting over.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct DraftFeedbackTests {
    private func started() -> DraftFeedback {
        var feedback = DraftFeedback()
        feedback.start(draftId: "draft-1")
        return feedback
    }

    @Test func thumbsUpIsKeptAtOnceWithNoReasonsAndNoContent() {
        var feedback = started()
        let rating = feedback.up()
        #expect(rating == DraftRating(verdict: .up, reasons: [], comment: ""))
        feedback.settle(rating, kept: true)
        #expect(feedback.stage == .kept(.up))
        #expect(feedback.verdict == .up)
        #expect(!feedback.includeContent)
    }

    @Test func thumbsDownAsksAndListsTheReasonsInTheFormsOrder() {
        var feedback = started()
        feedback.down()
        #expect(feedback.stage == .asking)
        #expect(!feedback.includeContent)
        feedback.reasons = [.somethingElse, .wrongTone, .madeThingsUp]
        feedback.comment = "  Too stiff for Bob.  "
        let rating = feedback.downRating()
        #expect(rating.verdict == .down)
        #expect(rating.reasons == [.wrongTone, .madeThingsUp, .somethingElse])
        #expect(rating.comment == "Too stiff for Bob.")
        feedback.cancel()
        #expect(feedback.stage == .offered)
        #expect(feedback.reasons.count == 3)
    }

    @Test func theCommentStopsAtTheCoresLimit() {
        var feedback = started()
        feedback.comment = String(repeating: "a", count: DraftFeedback.commentLimit + 20)
        #expect(feedback.comment.count == DraftFeedback.commentLimit)
    }

    @Test func aRatingTheCoreDidNotKeepSaysSo() {
        var feedback = started()
        feedback.settle(feedback.up(), kept: false)
        #expect(feedback.stage == .failed)
    }

    @Test func aLaterDraftStartsOver() {
        var feedback = started()
        feedback.down()
        feedback.reasons = [.wrongLanguage]
        feedback.includeContent = true
        feedback.settle(feedback.downRating(), kept: true)
        feedback.start(draftId: "draft-2")
        #expect(feedback.draftId == "draft-2")
        #expect(feedback.stage == .offered)
        #expect(feedback.reasons.isEmpty)
        #expect(!feedback.includeContent)
    }

    @Test func everyReasonHasItsOwnWords() {
        let titles = DraftFeedback.reasons.map(DraftFeedback.title)
        #expect(Set(titles).count == DraftFeedback.reasons.count)
        #expect(titles.first == L10n.ai_feedback_reason_tone())
    }
}
