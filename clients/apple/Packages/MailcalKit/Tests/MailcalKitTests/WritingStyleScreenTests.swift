// Settings → Writing style as drawn (SettingsWritingStyle.swift): nothing without the core's page,
// a row per style, Learn held with its reason and remedy, and a run drawn in Learn's place with its
// Stop. Hosted in a window of its own with a model holding the record, so no core is needed; the
// running app is `WritingStyleSettingsTests` in the UI suite.

#if os(iOS) && DEBUG
import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

@Suite @MainActor struct WritingStyleScreenTests {
    private func screen(_ page: WritingStylePage?) -> HostedScreen {
        let model = MailboxModel()
        model.writingStyle.page = page
        return HostedScreen(SettingsWritingStyle(model: model), size: CGSize(width: 393, height: 1400))
    }

    private func page(
        styles: [WritingStyleEntry] = [], learn: ControlState = .free(),
        blocked: WritingStyleLearnBlock? = nil, run: WritingStyleRun? = nil
    ) -> WritingStylePage {
        WritingStylePage(
            styles: styles, accounts: [], learn: learn, learnBlocked: blocked, run: run,
            outcome: nil)
    }

    private let style = WritingStyleEntry(
        id: "s1", name: "My style",
        facts: WritingStyleFacts(
            messages: 42, learnedFrom: "alice@test.local", since: nil, learnedAt: nil,
            languages: ["en"]),
        opens: .detail, open: .free())

    @Test func nothingIsDrawnWithoutThePage() {
        #expect(screen(nil).elements("Learn my writing style").isEmpty)
    }

    @Test func learnIsFreeWithNothingBlockingIt() {
        let shown = screen(page())
        #expect(shown.elements("Learn my writing style").first?.enabled == true, "\(shown.dump())")
    }

    @Test func aHeldLearnOffersItsOneRemedy() {
        let shown = screen(page(
            learn: .free(false),
            blocked: .cannotSend(
                failure: .nowhereToSend,
                remedy: AiRemedyControl(remedy: .openAiEndpoint, control: .free()))))
        #expect(shown.elements("Learn my writing style").first?.enabled == false, "\(shown.dump())")
        #expect(shown.elements(L10n.ai_remedy_open_endpoint()).first?.enabled == true)
    }

    @Test func aRunIsDrawnInLearnsPlaceWithItsStop() {
        let shown = screen(page(run: .learning(email: "alice@test.local", done: 1, total: 4, stop: .free())))
        #expect(shown.elements("Learn my writing style").isEmpty, "\(shown.dump())")
        #expect(shown.elements(L10n.learn_stop()).first?.enabled == true, "\(shown.dump())")
    }

    @Test func aStyleIsARowThatOpensIt() {
        let shown = screen(page(styles: [style]))
        #expect(
            !shown.elements(role: "AXButton") { $0.hasPrefix("My style") }.isEmpty,
            "\(shown.dump())")
    }
}
#endif
