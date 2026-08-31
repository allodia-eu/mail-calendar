// The two settings controls the categorised macOS screen and the simpler iOS sheet both mount:
// the default send account (which account new mail in the combined inbox goes out from) and the
// per-direction swipe actions. State lives in the Rust core (persisted preferences); these render
// it and dispatch the setters, which re-signal `Surface::Settings`. Their Android twins are
// SettingsComposing.kt, keep the wording and the rules in step.

import MailcalBindings
import SwiftUI

/// Which account new mail composes from when the combined inbox is showing. Only meaningful with
/// more than one account, so with a single account it states the sender instead of offering a
/// choice of one. The picker binds to the *stored* id, which may name an account that has since
/// been removed, `sendAccount(preferring:)` is what resolves that to a real one.
struct DefaultSendAccountPicker: View {
    var model: MailboxModel

    var body: some View {
        if model.accounts.count > 1 {
            Picker(L10n.settings_send_account_heading(), selection: binding) {
                ForEach(model.accounts, id: \.id) { account in
                    Text(account.email).tag(Optional(account.id))
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
        } else {
            Text(model.accounts.first?.email ?? L10n.settings_accounts_empty())
                .font(.callout)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
        }
    }

    /// Reads the *effective* account so the menu never opens on a blank row when the stored
    /// default names a removed account; writing always stores the explicit choice.
    private var binding: Binding<String?> {
        Binding(
            get: { model.sendAccount(preferring: model.defaultSendAccount)?.id },
            set: { model.setDefaultSendAccount($0) }
        )
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
