// The "your name" field on an account's page: what recipients see beside the address on the mail
// this account sends (docs/sending.md), as the core's page record says it.

import MailcalBindings
import SwiftUI

/// The name field for one account, or the note that the name is not this person's to change.
/// Drawn directly under the sharing control: the one thing on the page a recipient can see
/// (docs/settings.md).
struct SenderNameSection: View {
    var model: MailboxModel
    let accountId: String
    let name: AccountSenderName

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_sender_name_heading(),
            description: L10n.settings_sender_name_description()
        ) {
            if name.editable {
                ControlTextField(
                    title: L10n.settings_sender_name_heading(), text: name.name,
                    control: name.control
                ) { typed in
                    model.changeAccount(.setSenderName(account: accountId, name: typed))
                }
                #if os(iOS)
                    .textInputAutocapitalization(.words)
                    .autocorrectionDisabled()
                #endif
            } else {
                // The name is the organisation's. Show it, and say why there is no field, rather
                // than drawing a disabled box that reads as a bug.
                VStack(alignment: .leading, spacing: 4) {
                    Text(name.name)
                    Text(L10n.settings_sender_name_managed())
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
    }
}
