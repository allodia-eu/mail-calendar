// The two settings controls the categorised macOS screen and the simpler iOS sheet both mount:
// the default send account (which account a new message is written from when no mailbox on screen
// names one) and the per-direction swipe actions. State lives in the Rust core (persisted
// preferences); these render it and dispatch the setters, which re-signal `Surface::Settings`.
// Their Android twins are SettingsComposing.kt; keep the wording in step.

import MailcalBindings
import SwiftUI

/// Settings → Composing's default send account, drawn as the core decides it
/// (`sendAccountSetting`): no account, the one account's address, or a picker over the accounts
/// used for mail on the one a new message is written from.
struct DefaultSendAccountPicker: View {
    var model: MailboxModel

    var body: some View {
        switch model.sendAccountSetting {
        case .noAccount:
            sender(L10n.settings_accounts_empty())
        case let .only(email):
            sender(email)
        case let .choose(accounts, selected):
            Picker(L10n.settings_send_account_heading(), selection: binding(selected)) {
                ForEach(accounts, id: \.id) { account in
                    Text(account.email).tag(account.id)
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
        }
    }

    private func sender(_ text: String) -> some View {
        Text(text)
            .font(.callout)
            .foregroundStyle(.secondary)
            .lineLimit(1)
            .truncationMode(.middle)
    }

    private func binding(_ selected: String) -> Binding<String> {
        Binding(get: { selected }, set: { model.setDefaultSendAccount($0) })
    }
}

/// The two swipe directions, each an independent Trash / Archive / Star picker.
struct SwipeActionsPicker: View {
    var model: MailboxModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            direction(L10n.settings_swipe_left(), .left, model.swipeSettings.left)
            direction(L10n.settings_swipe_right(), .right, model.swipeSettings.right)
        }
    }

    /// The direction label is drawn here rather than left to the Picker: outside a Form, a `.menu`
    /// Picker renders its label on macOS but drops it on iOS, which would leave the two rows
    /// indistinguishable whenever both directions run the same action.
    @ViewBuilder
    private func direction(_ label: String, _ edge: SwipeDirection, _ selected: SwipeActionKind) -> some View {
        HStack(spacing: 8) {
            Text(label).foregroundStyle(.secondary)
            Picker(label, selection: binding(edge, selected)) {
                ForEach(swipeActionKinds, id: \.self) { action in
                    Label(swipeActionLabel(action), systemImage: swipeActionSymbol(action)).tag(action)
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .fixedSize()
            Spacer(minLength: 0)
        }
    }

    private func binding(_ edge: SwipeDirection, _ selected: SwipeActionKind) -> Binding<SwipeActionKind> {
        Binding(get: { selected }, set: { model.setSwipeAction(edge, $0) })
    }
}
