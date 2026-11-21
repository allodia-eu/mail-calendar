// Settings → Advanced (docs/settings.md, Advanced), drawn from the core's `AdvancedPage`: AI
// assistant access first, where the host set an endpoint, then the own AI endpoint's form from its
// own record while the writing assistant is offered, then the reset of the local database. Settings
// stays open through a reset: the button runs until mail is downloaded again, and what it came
// to is said beneath it.

import MailcalBindings
import SwiftUI

/// The words for what the last reset came to.
enum ResetOutcomeWords {
    static func text(_ outcome: ResetOutcome) -> String {
        switch outcome {
        case .done: L10n.settings_advanced_reset_done()
        case .failed: L10n.settings_advanced_reset_failed()
        }
    }
}

extension AdvancedPage {
    /// The page before the core has opened: no assistant panel and nothing to press.
    static var empty: AdvancedPage {
        AdvancedPage(assistants: nil, reset: .free(false), resetOutcome: nil)
    }
}

struct AdvancedSettingsView: View {
    var model: MailboxModel

    @State private var confirmingReset = false

    var body: some View {
        let page = model.advancedPage
        VStack(alignment: .leading, spacing: 20) {
            if let panel = page.assistants {
                McpSettingsView(model: model, panel: panel)
            }
            AiEndpointSettings(model: model)
            resetGroup(page)
        }
    }

    private func resetGroup(_ page: AdvancedPage) -> some View {
        SettingsGroup(
            heading: L10n.action_reset_database(),
            description: L10n.settings_advanced_reset_description()
        ) {
            VStack(alignment: .leading, spacing: 8) {
                ControlButton(
                    title: L10n.action_reset_database(), role: .destructive, control: page.reset
                ) { confirmingReset = true }
                if let outcome = page.resetOutcome {
                    Text(ResetOutcomeWords.text(outcome))
                        .font(.callout)
                        .foregroundStyle(outcome == .failed ? .red : .secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        // An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover, which
        // drops the `.cancel` button.
        .alert(L10n.reset_title(), isPresented: $confirmingReset) {
            Button(L10n.reset_confirm(), role: .destructive) { model.resetDatabase() }
            Button(L10n.action_cancel(), role: .cancel) {}
        } message: {
            Text(L10n.reset_message())
        }
    }
}
