// The step a JMAP sign-in ends on when its session offers more than one of mail, calendar and
// contacts (docs/onboarding.md, "After the sign-in"): which the account is used for, drawn with the
// found card's toggles. The rules are `SetupUsesStep`'s and `UseChoices`'; this draws them.

import MailcalBindings
import SwiftUI

struct SetupUsesStepView: View {
    var model: MailboxModel

    var body: some View {
        SetupScaffold {
            if let step = model.setupFlowShown.uses {
                Text(L10n.setup_signed_in_uses_title()).font(.title2).bold()
                    .fixedSize(horizontal: false, vertical: true)
                Text(L10n.setup_signed_in_uses_note())
                    .font(.callout).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                // Every use the session offers has its server, so no field is ever flagged.
                SetupUsesSection(
                    choices: Binding(
                        get: { model.setupFlowShown.uses?.choices ?? step.choices },
                        set: { model.setupFlow.choose($0) }),
                    flagged: .constant(.none)
                ) { EmptyView() }
                .disabled(model.savingSetupUses)
                Text(L10n.setup_links_note())
                    .font(.caption).foregroundStyle(.secondary)
                SetupFooter {
                    if model.savingSetupUses {
                        ProgressView().controlSize(.small)
                        Text(L10n.setup_signed_in_uses_saving()).foregroundStyle(.secondary)
                    } else {
                        Button(L10n.setup_detect_action()) {
                            Task { await model.setupUsesDone() }
                        }
                        .buttonStyle(.borderedProminent)
                        .keyboardShortcut(.defaultAction)
                    }
                }
            }
        }
    }
}
