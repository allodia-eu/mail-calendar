// The learning flow in the flow host (docs/ai.md rules 11 to 14): the stage the core is on and
// exactly the controls its record gives, each sent as the intent it names. Once the style is
// learned, the reveal follows in the same sheet.

import MailcalBindings
import SwiftUI

struct WritingStyleLearningSheet: View {
    var model: MailboxModel

    var body: some View {
        if let flow = model.writingStyle.learning {
            if case let .learned(style) = flow.stage {
                // The reveal's own Close ends the flow too: the core closes it on `closeReveal`.
                WritingStyleRevealView(model: model, styleId: style) {}
            } else {
                host(flow)
            }
        }
    }

    private func host(_ flow: WritingStyleLearning) -> some View {
        let key = StyleLearningStageKey(flow.stage)
        return FlowHost(
            trail: flow.trail.map(StyleLearningStageKey.init) + [key],
            title: title,
            cancel: FlowControl(title: closeTitle(flow.stage), control: flow.close) {
                model.learnStyle(.close)
            },
            back: flow.back.map { control in
                FlowControl(title: L10n.wizard_back(), control: control) { model.learnStyle(.back) }
            },
            primary: primary(flow)
        ) { shown in
            // Only the stage on screen has its record; one beneath it in the stack is its title
            // until Back makes it current again.
            if shown == key {
                WritingStyleLearningStage(model: model, stage: flow.stage)
            }
        }
        .interactiveDismissDisabled(key == .learning)
    }

    /// The one forward action the stage offers: Learn on the review, which is the consent; Next;
    /// the failure's remedy; Stop while a run goes.
    private func primary(_ flow: WritingStyleLearning) -> FlowControl? {
        if let learn = flow.learn {
            return FlowControl(title: L10n.learn_consent_confirm(), control: learn) {
                model.learnStyle(.learn(
                    name: L10n.writing_style_default_name(), language: Self.language))
            }
        }
        if let next = flow.next {
            return FlowControl(title: L10n.wizard_next(), control: next) { model.learnStyle(.next) }
        }
        if case let .failed(_, remedy?) = flow.stage {
            return FlowControl(title: AiWords.remedy(remedy.remedy), control: remedy.control) {
                model.openRemedy(remedy.remedy, inFlow: true)
            }
        }
        if let stop = flow.stop {
            return FlowControl(title: L10n.learn_stop(), control: stop, role: .destructive) {
                model.learnStyle(.stop)
            }
        }
        return nil
    }

    private func title(_ stage: StyleLearningStageKey) -> String {
        switch stage {
        case .chooseAccount: L10n.learn_account_title()
        case .chooseRange: L10n.learn_range_title()
        case .review: L10n.learn_review_title()
        case .learning, .learned: L10n.settings_category_writing_style()
        case .failed: L10n.learn_failed_title()
        }
    }

    /// Cancel while nothing has been learned; Close while a run goes, which it leaves running on
    /// the page, and after a failure.
    private func closeTitle(_ stage: StyleLearningStage) -> String {
        switch stage {
        case .learning, .failed, .learned: L10n.action_close()
        case .chooseAccount, .chooseRange, .review: L10n.action_cancel()
        }
    }

    /// The language the app is read in, as an ISO 639-1 code: what the description is written in.
    static var language: String {
        L10n.appLocale.language.languageCode?.identifier ?? "en"
    }
}
