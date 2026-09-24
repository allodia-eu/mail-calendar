// What a person says about a drafted reply (docs/ai.md, "Feedback"), and what the training window
// says about each draft it compares: thumbs up at once, thumbs down through a short form. Held as a
// plain value so its rules are tested without drawing the controls.

import Foundation
import MailcalBindings

struct DraftFeedback: Equatable {
    enum Stage: Equatable {
        /// The thumbs are showing.
        case offered
        /// The form behind thumbs down is open.
        case asking
        /// A rating was kept; feedback on a draft thanks the person and offers nothing more.
        case kept(DraftVerdict)
        /// The core did not keep it.
        case failed
    }

    /// Every reason, in the order the form lists them.
    static let reasons: [DraftRatingReason] = [
        .wrongTone, .wrongLength, .madeThingsUp, .missedThePoint, .wrongLanguage, .somethingElse,
    ]
    /// The longest comment the form takes, as the core keeps it.
    static let commentLimit = 1000

    private(set) var stage = Stage.offered
    /// The draft rated, for feedback on a composer's draft; `nil` in the training window.
    private(set) var draftId: String?
    var reasons: Set<DraftRatingReason> = []
    var comment = "" {
        didSet {
            if comment.count > Self.commentLimit { comment = String(comment.prefix(Self.commentLimit)) }
        }
    }
    /// Whether the email and the draft go with the rating. Never ticked until the person ticks it.
    var includeContent = false

    /// Starts again for a new draft.
    mutating func start(draftId: String?) {
        self = DraftFeedback()
        self.draftId = draftId
    }

    /// A thumbs up: kept at once, with no reasons and nothing of the draft.
    func up() -> DraftRating {
        DraftRating(verdict: .up, reasons: [], comment: "")
    }

    /// A thumbs down opens the form.
    mutating func down() {
        stage = .asking
    }

    mutating func cancel() {
        stage = .offered
    }

    /// The form's rating, its reasons in the form's order.
    func downRating() -> DraftRating {
        DraftRating(
            verdict: .down,
            reasons: Self.reasons.filter(reasons.contains),
            comment: comment.trimmingCharacters(in: .whitespacesAndNewlines)
        )
    }

    /// What became of `rating`.
    mutating func settle(_ rating: DraftRating, kept: Bool) {
        stage = kept ? .kept(rating.verdict) : .failed
    }

    /// The verdict last kept, which the training window draws filled in.
    var verdict: DraftVerdict? {
        if case let .kept(verdict) = stage { verdict } else { nil }
    }

    /// What the form calls a reason.
    static func title(_ reason: DraftRatingReason) -> String {
        switch reason {
        case .wrongTone: L10n.ai_feedback_reason_tone()
        case .wrongLength: L10n.ai_feedback_reason_length()
        case .madeThingsUp: L10n.ai_feedback_reason_made_up()
        case .missedThePoint: L10n.ai_feedback_reason_missed_point()
        case .wrongLanguage: L10n.ai_feedback_reason_language()
        case .somethingElse: L10n.ai_feedback_reason_other()
        }
    }
}
