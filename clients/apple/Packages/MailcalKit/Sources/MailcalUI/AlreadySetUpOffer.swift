// Beneath the "This account is already set up" refusal on the setup form: the offer to put the
// credential just given on the account already there (docs/accounts.md rules 15 and 17).

import MailcalBindings
import SwiftUI

struct AlreadySetUpOffer: View {
    var model: MailboxModel

    var body: some View {
        if model.setupExistingAccount != nil {
            VStack(alignment: .leading, spacing: 6) {
                if model.updatingExistingAccount {
                    progress(L10n.status_connecting())
                } else {
                    Button(L10n.account_already_set_up_update()) { model.updateExistingAccount() }
                        .buttonStyle(.bordered)
                }
                Text(L10n.account_already_set_up_update_note())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}
