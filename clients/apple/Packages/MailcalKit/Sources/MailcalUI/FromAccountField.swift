// The composer's From dropdown: pick which configured account a message is sent from. The core
// sends as, and through, that account (its identity AND its outbox), so this is an account
// picker, not a free-text From header.
//
// The caller decides which account it opens on (`MailboxModel.sendAccount(preferring:)`): the one
// that received the mail being replied to/forwarded, else the selected mailbox's account, else the
// app-level default send account. Kept in its own file so RichComposerView.swift stays small.

import MailcalBindings
import SwiftUI

struct FromAccountField: View {
    let accounts: [AccountRow]
    @Binding var selection: String?

    var body: some View {
        if !accounts.isEmpty {
            // The label is drawn here rather than left to the Picker: outside a Form, a `.menu`
            // Picker renders its label on macOS but drops it on iOS, and the From address must
            // never appear as a bare unlabeled email.
            HStack(spacing: 8) {
                Text(L10n.compose_from()).foregroundStyle(.secondary)
                field
                Spacer(minLength: 0)
            }
        }
    }

    /// With one account there is nothing to choose, so the field renders as plain read-only text
    /// rather than a menu that opens onto a single item. It stays visible either way.
    @ViewBuilder
    private var field: some View {
        if accounts.count > 1 {
            Picker(L10n.compose_from(), selection: $selection) {
                ForEach(accounts, id: \.id) { account in
                    Text(account.email).tag(Optional(account.id))
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .fixedSize()
        } else if let only = accounts.first {
            Text(only.email).lineLimit(1).truncationMode(.middle)
        }
    }
}
