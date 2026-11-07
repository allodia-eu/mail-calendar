// The last setup step, and a skippable one (docs/onboarding.md, "After the connect"): an account
// that lacks a calendar or contacts can use another account's, and a calendar without mail can
// name the account that sends its invitations. The pickers are the core's: the ones the account's
// page offers that nothing fills yet.

import MailcalBindings
import SwiftUI

struct SetupLinkStepView: View {
    var model: MailboxModel

    var body: some View {
        SetupScaffold {
            if let step = model.setupFlow.links {
                Text(L10n.setup_links_title()).font(.title2).bold()
                Text(L10n.settings_account_links_description())
                    .font(.callout).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                if !step.pickers.isEmpty {
                    SetupCard {
                        VStack(alignment: .leading, spacing: 12) {
                            ForEach(Array(step.pickers.enumerated()), id: \.element.slot) { index, picker in
                                // A suggestion starts picked, so it is named without a button: the
                                // person confirms it by continuing.
                                LinkPickerRow(
                                    picker: picker, selected: step.picked[index], suggestButton: false,
                                    control: .free(!model.savingSetupLinks)
                                ) { target in
                                    model.setupFlow.pick(
                                        index, target.flatMap { id in picker.options.firstIndex { $0.id == id } })
                                }
                            }
                        }
                    }
                }
                if let beside = step.beside {
                    Text(beside.fills.note(beside.offer.host))
                        .font(.callout)
                        .fixedSize(horizontal: false, vertical: true)
                    Button(beside.fills.action(beside.offer.host)) { model.setupBesideAccount() }
                        .disabled(model.savingSetupLinks)
                }
                Text(L10n.setup_links_note())
                    .font(.caption).foregroundStyle(.secondary)
                Button(L10n.setup_links_add()) { model.setupAddLinkedAccount() }
                    .disabled(model.savingSetupLinks)
                SetupFooter {
                    Button(L10n.setup_sender_name_skip()) {
                        Task { await model.setupLinksDone(link: false) }
                    }
                    .keyboardShortcut(.cancelAction)
                    if model.savingSetupLinks {
                        ProgressView().controlSize(.small)
                    } else {
                        Button(L10n.setup_detect_action()) {
                            Task { await model.setupLinksDone(link: true) }
                        }
                        .buttonStyle(.borderedProminent)
                        .keyboardShortcut(.defaultAction)
                    }
                }
                .disabled(model.savingSetupLinks)
            }
        }
    }
}
