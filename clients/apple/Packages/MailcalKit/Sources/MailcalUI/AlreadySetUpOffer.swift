// Beneath the "This account is already set up" refusal on the setup form: the offer to put the
// credential just given on the account already there (docs/accounts.md rules 15 and 17), asking
// first when that credential would not reach the account's calendar or contacts (rule 18).

import MailcalBindings
import SwiftUI

struct AlreadySetUpOffer: View {
    var model: MailboxModel

    @State private var question: UnreachedQuestion?

    var body: some View {
        if model.setupExistingAccount != nil {
            VStack(alignment: .leading, spacing: 6) {
                if model.updatingExistingAccount {
                    progress(L10n.status_connecting())
                } else {
                    Button(L10n.account_already_set_up_update()) {
                        model.updateExistingAccount { set in
                            await UnreachedQuestion.ask(set) { question = $0 }
                        }
                    }
                    .buttonStyle(.bordered)
                }
                Text(L10n.account_already_set_up_update_note())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .unreachedUsesAlert($question)
        }
    }
}
