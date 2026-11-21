// What the Apple clients draw of the writing assistant follows the core's offer (docs/ai.md rule
// 4): Settings lists Writing style only while its page is there, opens on or stays on it only then,
// and a composer puts Draft a reply's input and questions away when its record goes. The offer is
// read once at launch, so withdrawing it is staged here by taking the records away.

import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

@MainActor
private func offered(_ model: MailboxModel, _ on: Bool) {
    model.writingStyle.page = on
        ? WritingStylePage(
            styles: [], accounts: [], learn: .free(), learnBlocked: nil, run: nil, outcome: nil)
        : nil
}

private func record() -> ComposerReplyDraft {
    ComposerReplyDraft(
        state: .ready, control: .free(), draft: .free(), stop: .free(false), heldBy: nil,
        suggestions: [.yes], instruction: "", askBeforeReplacing: true, replaceArrived: false,
        editorScript: nil, sendQuestion: nil)
}

@MainActor
@Suite struct WritingAssistantOfferTests {
    @Test func settingsListsWritingStyleOnlyWhileOffered() {
        // The Mac's sidebar and the iPad's are drawn from this list.
        let model = AppSession().model
        offered(model, false)
        #expect(!SettingsCategory.displayed(model).contains(.writingStyle))
        #expect(SettingsCategory.reachable(.writingStyle, in: model) == .general)
        offered(model, true)
        #expect(SettingsCategory.displayed(model).contains(.writingStyle))
        #expect(SettingsCategory.reachable(.writingStyle, in: model) == .writingStyle)
    }

    @Test func aComposerPutsDraftAReplyAwayWhenItsRecordGoes() {
        var ui = ReplyDraftUI()
        ui.take(record())
        ui.inputShown = true
        ui.askingReplace = true
        ui.askingSend = true
        ui.take(record())
        #expect(ui.inputShown && ui.askingReplace && ui.askingSend, "a record that stays keeps them")

        ui.take(nil)
        #expect(ui.record == nil)
        #expect(!ui.inputShown, "the input would reopen on a later grant")
        #expect(!ui.askingReplace, "Replace would still draft")
        #expect(!ui.askingSend)
    }

    #if os(iOS) && DEBUG
    // On a phone the category's page is pushed, so it is shown exactly while the system's back
    // button is. The iPad's split and the Mac's sidebar select through the same `reachable`, and
    // a hosted window gives the agent nothing inside their detail pane's scroll view.
    @Test func thePhonesSettingsLeaveWritingStyleWhenTheOfferGoes() {
        let model = AppSession().model
        offered(model, true)
        let shown = hub(model)
        #expect(pushed(shown), "\(shown.dump())")
        offered(model, false)
        shown.settle()
        #expect(!pushed(shown), "\(shown.dump())")
    }

    @Test func thePhonesSettingsAskedForWritingStyleWhileNotOfferedOpenOnTheHub() {
        let model = AppSession().model
        offered(model, false)
        let shown = hub(model)
        #expect(!pushed(shown), "\(shown.dump())")
    }

    private func hub(_ model: MailboxModel) -> HostedScreen {
        HostedScreen(
            SettingsHubView(model: model, close: {}, removeAccount: { _ in }, openOn: .writingStyle),
            compact: true)
    }

    /// The system's back button: a chevron with no text, a `Button` in the tree.
    private func pushed(_ shown: HostedScreen) -> Bool {
        shown.dump().split(separator: "\n").contains { $0.split(separator: " ") == ["Button", "AXButton"] }
    }
    #endif
}
