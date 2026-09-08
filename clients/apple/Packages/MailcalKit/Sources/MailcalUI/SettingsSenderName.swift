// The "your name" field on an account's settings card: what recipients see beside the address
// on the mail this account sends (docs/sending.md).
//
// Its own file so SettingsCategoryDetail stays under the 500-line limit, and because the field
// is the one control on the card a stranger can see the effect of: everything else there is
// about how much of the account this device keeps.

import MailcalBindings
import SwiftUI

/// The name field for one account, or the note that the name is not this person's to change.
struct SenderNameSection: View {
    var model: MailboxModel
    let account: AccountSyncRow

    /// The text being edited, seeded from the snapshot.
    ///
    /// Local while the field has focus, because committing on every keystroke would push a
    /// half-typed name to the provider and rebuild the snapshot underneath the cursor. The
    /// core is told on commit, which on both platforms is losing focus or pressing return.
    @State private var draft: String = ""
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(L10n.settings_sender_name_heading()).font(.subheadline).bold()
            Text(L10n.settings_sender_name_description())
                .font(.caption)
                .foregroundStyle(.secondary)
            if account.senderNameEditable {
                TextField(L10n.settings_sender_name_heading(), text: $draft)
                    .textFieldStyle(.roundedBorder)
                    .labelsHidden()
                    .focused($focused)
                    .frame(maxWidth: 320, alignment: .leading)
                    .onSubmit { commit() }
                    .onChange(of: focused) { _, isFocused in
                        if !isFocused { commit() }
                    }
                    #if os(iOS)
                        .textInputAutocapitalization(.words)
                        .autocorrectionDisabled()
                    #endif
            } else {
                // The name is the organisation's. Show it, and say why there is no field,
                // rather than drawing a disabled box that reads as a bug.
                Text(account.senderName.isEmpty ? account.email : account.senderName)
                Text(L10n.settings_sender_name_managed())
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .onAppear { draft = account.senderName }
        // A snapshot can arrive while this card is on screen (another surface changed it, or
        // the push landed). Adopt it unless the field is being typed in.
        .onChange(of: account.senderName) { _, latest in
            if !focused { draft = latest }
        }
    }

    /// Sends the edit to the core, which sanitises and persists it. Skipped when nothing
    /// changed, so tabbing through the card costs no write.
    private func commit() {
        guard draft != account.senderName else { return }
        model.setAccountSenderName(account.accountId, draft)
    }
}
