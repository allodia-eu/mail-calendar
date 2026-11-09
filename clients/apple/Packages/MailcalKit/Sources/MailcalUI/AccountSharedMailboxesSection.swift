// The Shared mailboxes section of an account's page (docs/shared-mailboxes.md rules 3, 7 and 8),
// drawn from the page's shared mailboxes: a switch per mailbox the account's sign-in opens besides
// its own, the line under them and, on a server that lists none, a field for one's address.
// Switching one off is confirmed first, because it deletes what the device holds of it.

import MailcalBindings
import SwiftUI

struct AccountSharedMailboxesSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let shared: AccountSharedMailboxes

    @State private var confirmingOff: AccountSharedMailbox?

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_account_shared_heading(),
            description: L10n.settings_account_shared_description()
        ) {
            VStack(alignment: .leading, spacing: 10) {
                ForEach(shared.mailboxes, id: \.handle) { mailbox in
                    ControlToggle(
                        title: mailbox.title,
                        note: mailbox.withdrawn
                            ? L10n.settings_account_shared_withdrawn() : mailbox.label,
                        on: mailbox.on,
                        control: mailbox.control
                    ) { on in
                        if on { set(mailbox, on: true) } else { confirmingOff = mailbox }
                    }
                }
                if let field = shared.address {
                    SharedAddressField(model: model, accountId: entry.id, field: field)
                }
                if let note = shared.note {
                    Text(AccountSettingsLabels.sharedNote(note))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .confirmAlert(
            $confirmingOff, action: L10n.action_switch_off(),
            title: { L10n.settings_account_shared_off_title(mailbox: $0.title) },
            message: { _ in L10n.settings_account_shared_off_message() },
            confirm: { set($0, on: false) })
    }

    private func set(_ mailbox: AccountSharedMailbox, on: Bool) {
        model.changeAccount(
            .setSharedMailbox(account: entry.id, handle: mailbox.handle, on: on))
    }
}

/// The address of a mailbox shared with the account, for a server that lists none, drawn with
/// the address being added or the one the last add found nothing at. Sent with Return or Add,
/// never per keystroke: each asks the server.
private struct SharedAddressField: View {
    var model: MailboxModel
    let accountId: String
    let field: AccountSharedAddress

    @State private var typed: String

    init(model: MailboxModel, accountId: String, field: AccountSharedAddress) {
        self.model = model
        self.accountId = accountId
        self.field = field
        _typed = State(initialValue: field.text)
    }

    var body: some View {
        let address = typed.trimmingCharacters(in: .whitespaces)
        HStack(spacing: 8) {
            TextField(L10n.settings_account_shared_address(), text: $typed)
                .setupField(.email)
                .onSubmit { add(address) }
            ControlButton(
                title: L10n.action_add(), control: field.add, ready: !address.isEmpty
            ) { add(address) }
        }
        .onChange(of: field.text) { _, text in typed = text }
    }

    private func add(_ address: String) {
        guard !address.isEmpty, field.add.enabled else { return }
        model.changeAccount(.addSharedMailbox(account: accountId, address: address))
    }
}
