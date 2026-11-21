// What Settings → Writing style does with the core's records that is not drawing (docs/ai.md rules
// 8 to 16): which outcome a surface acts on, once; when the detail's Save is held; where a remedy
// goes; and the stage a flow host is keyed by. Plain types, so the unit suite reads them without a
// screen.

import MailcalBindings

/// The page's hold on the library's outcomes: it says a refused change to an account's style,
/// once. A reveal or a detail says its own.
struct WritingStylePageSession {
    private var seen: UInt64

    init(page: WritingStylePage?) {
        seen = page?.outcome?.serial ?? 0
    }

    /// The words for `outcome` when the page should say it, once.
    mutating func take(_ outcome: WritingStyleOutcome?) -> String? {
        guard let outcome, outcome.serial > seen else { return nil }
        seen = outcome.serial
        guard outcome.kind == .assign, case let .refused(reason) = outcome.result else {
            return nil
        }
        return WritingStyleWords.refusal(reason)
    }
}

/// What a reveal or a detail does with a change it sent.
enum WritingStyleSheetVerdict: Equatable {
    /// Made, written or not: the sheet closes. A change not written is the failed-save notice's.
    case close
    /// Made, and the sheet stays: a rename or the notes, from the detail.
    case stay
    /// Not made: the sheet stays and says why.
    case refused(String)
}

/// One sheet's hold on the library's outcomes: the style it shows and the newest outcome it has
/// seen.
struct WritingStyleSheetSession {
    let style: String
    private var seen: UInt64

    /// A sheet opening on `style` now, which an outcome already on `page` does not concern.
    init(style: String, page: WritingStylePage?) {
        self.style = style
        seen = page?.outcome?.serial ?? 0
    }

    /// What to do with `outcome`, once: `nil` for one seen, or for another style's.
    mutating func take(_ outcome: WritingStyleOutcome?) -> WritingStyleSheetVerdict? {
        guard let outcome, outcome.serial > seen else { return nil }
        seen = outcome.serial
        guard outcome.style == style else { return nil }
        switch outcome.result {
        case let .refused(reason):
            return .refused(WritingStyleWords.refusal(reason))
        case .done, .notWritten:
            switch outcome.kind {
            case .saveReveal, .closeReveal, .forget: return .close
            case .rename, .notes, .assign: return .stay
            }
        }
    }
}

extension WritingStyleDetail {
    /// The detail's Save for the name typed: the record's Rename, held besides while the core
    /// refuses the name. The name the style already has is no refusal.
    func nameSave(_ name: String) -> ControlState {
        rename.held(writingStyleNameRefusal(name: name) != nil)
    }
}

/// Where a remedy goes. A remedy that moves the flow is the flow's intent; the others open what
/// they name.
enum WritingStyleRemedyRoute: Equatable {
    /// `StyleLearningIntent::Remedy`.
    case flow
    /// The Allodia sign-in.
    case allodiaSignIn
    /// Settings → Advanced, where the own endpoint's form is.
    case endpointForm
    /// Learn, on the page.
    case learn
    /// The page's style for each account.
    case accountStyles

    init(_ remedy: AiRemedy) {
        switch remedy {
        case .chooseAccount, .chooseRange, .tryAgain: self = .flow
        case .signInToAllodia: self = .allodiaSignIn
        case .openAiEndpoint: self = .endpointForm
        case .learnStyle: self = .learn
        case .chooseStyle: self = .accountStyles
        }
    }
}

/// A stage of the learning flow as the flow host keys it: which stage, not what it holds, so the
/// host pushes a stage once and redraws it in place as its content moves.
enum StyleLearningStageKey: Hashable {
    case chooseAccount, chooseRange, review, learning, failed, learned

    init(_ stage: StyleLearningStage) {
        switch stage {
        case .chooseAccount: self = .chooseAccount
        case .chooseRange: self = .chooseRange
        case .review: self = .review
        case .learning: self = .learning
        case .failed: self = .failed
        case .learned: self = .learned
        }
    }
}

extension StyleLearningStageKey {
    /// A stage the record's trail names.
    init(_ stage: StyleLearningTrailStage) {
        switch stage {
        case .chooseAccount: self = .chooseAccount
        case .chooseRange: self = .chooseRange
        case .review: self = .review
        }
    }
}

extension StyleLearnRange {
    /// Whether this is the same preset as `other`, whatever days a range of the person's own has.
    func samePreset(as other: StyleLearnRange?) -> Bool {
        switch (self, other) {
        case (.lastTwelveMonths, .lastTwelveMonths?), (.everything, .everything?),
             (.untilEndOfLastYear, .untilEndOfLastYear?), (.between, .between?):
            true
        default:
            false
        }
    }
}
