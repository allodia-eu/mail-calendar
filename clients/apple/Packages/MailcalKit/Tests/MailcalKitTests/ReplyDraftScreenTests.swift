// Draft a reply's input and card as drawn (ReplyDraftViews.swift), from records built here: Draft
// held and running as the record says, a held draft's reason and remedy in place of the field, and
// the card's ticks, a gap's held and the person's free. Hosted in a window of its own, so no core
// is needed; the composer in the running app is `ComposerDraftReplyTests` in the UI suite.

#if os(iOS) && DEBUG
import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

@Suite @MainActor struct ReplyDraftScreenTests {
    private func record(
        draft: ControlState = .free(), held: ReplyDraftHold? = nil
    ) -> ComposerReplyDraft {
        ComposerReplyDraft(
            state: .ready, control: .free(), draft: draft, stop: .free(false), heldBy: held,
            suggestions: [.yes, .no, .moreInformation, .holding], instruction: "",
            askBeforeReplacing: false, replaceArrived: false, editorScript: nil, sendQuestion: nil)
    }

    private func input(_ record: ComposerReplyDraft) -> HostedScreen {
        HostedScreen(ReplyDraftInput(
            record: record, line: .constant(""), draft: {}, remedy: { _ in }))
    }

    @Test func draftIsFreeAndEverySuggestionIsOffered() {
        let shown = input(record())
        #expect(shown.elements(L10n.composer_draft_create()).first?.enabled == true, "\(shown.dump())")
        for words in [L10n.composer_draft_chip_yes(), L10n.composer_draft_chip_later()] {
            #expect(!shown.elements(words).isEmpty, "\(words) is not offered: \(shown.dump())")
        }
    }

    @Test func aRunningDraftHoldsItsButton() {
        let shown = input(record(draft: ControlState(enabled: false, running: true)))
        #expect(shown.elements(L10n.composer_draft_create()).first?.enabled == false)
    }

    @Test func aHeldDraftSaysWhyWithItsRemedyInPlaceOfTheField() {
        let held = ReplyDraftHold(
            failure: .noStyle, remedy: AiRemedyControl(remedy: .learnStyle, control: .free()))
        let shown = input(record(held: held))
        #expect(shown.elements(L10n.composer_draft_create()).isEmpty, "\(shown.dump())")
        #expect(shown.elements(L10n.ai_remedy_learn_style()).first?.enabled == true)
    }

    @Test func aGapsTickFollowsTheReplyAndTheOthersAreThePersons() {
        let checklist = [
            ReplyDraftItem(index: 0, kind: .fillIn, text: "[date]", done: false, tick: nil),
            ReplyDraftItem(index: 1, kind: .do, text: "Book the room", done: false, tick: .free()),
        ]
        let shown = HostedScreen(ReplyDraftCard(
            summary: "They ask for a date.", checklist: checklist, tick: { _, _ in }))
        // The person's item is a toggle; a gap is a mark, read as its text and nothing to press.
        let lines = shown.dump().split(separator: "\n")
        #expect(
            lines.contains { $0.contains("Book the room") && $0.contains("AXSwitch") },
            "the person's item is not a toggle: \(lines)")
        #expect(
            lines.contains { $0.contains("[date]") && $0.contains("AXStaticText") },
            "a gap is drawn as a control: \(lines)")
    }
}
#endif
