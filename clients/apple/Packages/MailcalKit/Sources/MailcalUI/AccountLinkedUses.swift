// The links on an account's page (docs/accounts.md rule 12): a use a link fills, drawn as the
// account it comes from, and the accounts taking a use from this one, each with Stop linking.

import MailcalBindings
import SwiftUI

/// A use a link fills: a row naming the account it comes from, which opens that account's page.
struct LinkedUseRow: Equatable {
    let title: String
    let note: String
    /// The account whose page it opens.
    let opens: String
    /// Held while a change runs on the page.
    let enabled: Bool

    /// The row for `toggle`, or `nil` when the use is the account's own switch.
    static func of(_ toggle: AccountUseSwitch) -> LinkedUseRow? {
        guard let linked = toggle.linked else { return nil }
        return LinkedUseRow(
            title: AccountSettingsLabels.use(toggle.capability),
            note: AccountSettingsLabels.linkedNote(linked),
            opens: linked.id,
            enabled: toggle.control.enabled)
    }
}

/// Stop linking, as it is asked before it runs: what the confirmation says, and the change its
/// confirm sends.
struct StopLinkingAsk: Equatable {
    let title: String
    let message: String
    let intent: AccountPageIntent

    /// Asks about stopping every link between `account` and `taking`, which takes a use from it.
    static func of(_ taking: AccountLinkedFrom, on account: String) -> StopLinkingAsk {
        StopLinkingAsk(
            title: L10n.settings_account_stop_linking_title(
                account: AccountSettingsLabels.name(taking.account)),
            message: AccountSettingsLabels.stopLinkingMessage(taking),
            intent: .stopLinking(account: account, from: taking.account.id))
    }
}

/// The accounts taking a use from this one, each with what it takes and Stop linking, confirmed
/// first as a removal is.
struct AccountLinkedFromSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let linked: [AccountLinkedFrom]

    @State private var asking: StopLinkingAsk?

    var body: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Text(L10n.settings_account_linked_from_heading()).font(.headline)
                ForEach(Array(linked.enumerated()), id: \.element.account.id) { index, taking in
                    if index > 0 { Divider() }
                    row(taking)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
        .confirmAlert(
            $asking, action: L10n.settings_account_stop_linking(),
            title: { $0.title }, message: { $0.message },
            confirm: { model.changeAccount($0.intent) })
    }

    private func row(_ taking: AccountLinkedFrom) -> some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(AccountSettingsLabels.name(taking.account))
                Text(AccountSettingsLabels.linkedFromLine(taking))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .accessibilityElement(children: .combine)
            Spacer(minLength: 0)
            ControlButton(title: L10n.settings_account_stop_linking(), control: taking.stop) {
                asking = StopLinkingAsk.of(taking, on: entry.id)
            }
            .buttonStyle(.bordered)
        }
    }
}
