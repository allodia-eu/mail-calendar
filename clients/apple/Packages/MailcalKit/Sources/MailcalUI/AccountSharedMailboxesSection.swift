// The Shared mailboxes section of an account's page (docs/shared-mailboxes.md rules 3, 7 and 8): a
// switch per mailbox the account's sign-in opens besides its own, and on Microsoft a field for the
// address of one. The rules are `SharedMailboxRules`'; this draws them.

import MailcalBindings
import SwiftUI

struct AccountSharedMailboxesSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    /// What the server answered, kept by the page so the section can come and go with it.
    @Binding var answer: SharedMailboxesView?
    @Binding var alert: AccountPageAlert?

    /// The handle of the mailbox being switched, or of none.
    @State private var switching: String?
    @State private var confirmingOff: SharedMailboxEntry?
    @State private var typed = ""
    @State private var adding = false

    var body: some View {
        if let answer, SharedMailboxRules.shows(answer) {
            SettingsGroup(
                heading: L10n.settings_account_shared_heading(),
                description: L10n.settings_account_shared_description()
            ) {
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(answer.mailboxes, id: \.handle) { mailbox in
                        row(mailbox)
                    }
                    if answer.support == .byAddress { addressField }
                    if let note = SharedMailboxRules.note(answer) {
                        Text(note)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
            // An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover,
            // which drops the `.cancel` button (see the remove-account alert in Mailcal.swift).
            .alert(
                L10n.settings_account_shared_off_title(
                    mailbox: confirmingOff.map { SharedMailboxRules.row($0).title } ?? ""),
                isPresented: Binding(
                    get: { confirmingOff != nil }, set: { if !$0 { confirmingOff = nil } }
                ),
                presenting: confirmingOff
            ) { mailbox in
                Button(L10n.action_switch_off(), role: .destructive) { change(mailbox, on: false) }
                Button(L10n.action_cancel(), role: .cancel) {}
            } message: { _ in
                Text(L10n.settings_account_shared_off_message())
            }
        }
    }

    private func row(_ mailbox: SharedMailboxEntry) -> some View {
        let shown = SharedMailboxRules.row(mailbox)
        return HStack {
            Toggle(
                isOn: Binding(
                    get: { shown.active },
                    set: { on in
                        if on { change(mailbox, on: true) } else { confirmingOff = mailbox }
                    }
                )
            ) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(shown.title)
                    if let subtitle = shown.subtitle {
                        Text(subtitle).font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
            .toggleStyle(.switch)
            .disabled(switching != nil || adding)
            if switching == mailbox.handle {
                ProgressView().controlSize(.small)
            }
        }
    }

    /// Committed with Return or Add, never per keystroke: each commit asks the server.
    private var addressField: some View {
        HStack(spacing: 8) {
            TextField(L10n.settings_account_shared_address(), text: $typed)
                .setupField(.email)
                .onSubmit(add)
                .disabled(adding)
            Button(L10n.action_add(), action: add)
                .disabled(SharedMailboxRules.address(typed) == nil || adding || switching != nil)
            if adding {
                ProgressView().controlSize(.small)
            }
        }
    }

    private func change(_ mailbox: SharedMailboxEntry, on: Bool) {
        switching = mailbox.handle
        let id = entry.id
        Task {
            let failure = await model.setSharedMailbox(id, mailbox, on: on)
            answer = await model.sharedMailboxes(id)
            switching = nil
            if let failure {
                alert = AccountPageAlert(
                    title: L10n.settings_account_change_failed_title(), detail: failure)
            }
        }
    }

    /// A refusal leaves the address where it was typed, to correct.
    private func add() {
        guard let address = SharedMailboxRules.address(typed), !adding, switching == nil else {
            return
        }
        adding = true
        let id = entry.id
        Task {
            let added = await model.addSharedMailbox(id, address: address)
            answer = await model.sharedMailboxes(id)
            adding = false
            switch added {
            case .added:
                typed = ""
            case .notFound:
                alert = AccountPageAlert(
                    title: L10n.settings_account_change_failed_title(),
                    detail: L10n.settings_account_shared_not_found())
            case let .failed(detail):
                alert = AccountPageAlert(
                    title: L10n.settings_account_change_failed_title(), detail: detail)
            }
        }
    }
}

/// Asks the server which mailboxes the account's sign-in opens when the page opens, and again when
/// the answer may have changed: the account has connected since, or what it is used for changed.
struct SharedMailboxesAsking: ViewModifier {
    var model: MailboxModel
    let entry: AccountEntry
    @Binding var answer: SharedMailboxesView?

    @State private var asking = false

    func body(content: Content) -> some View {
        content
            // The page is drawn per account (`.id(entry.id)`), so this is once per account.
            .task { await ask() }
            // A use switched on or off changes what the sign-in may open, and switching mail or
            // shared mailboxes off removes every chosen one (docs/shared-mailboxes.md rule 2).
            .onChange(of: AccountSettingsRules.inUse(entry)) { Task { await ask() } }
            .onChange(of: entry) { askAgain() }
            .onChange(of: model.accounts) { askAgain() }
            .onChange(of: model.connectivity) { askAgain() }
    }

    private func askAgain() {
        guard !asking, SharedMailboxRules.mayAskAgain(answer) else { return }
        // Claimed before the task starts: one snapshot changes `entry`, `accounts` and
        // `connectivity` together, and each would otherwise start an ask of its own.
        asking = true
        Task { await ask() }
    }

    private func ask() async {
        asking = true
        let before = answer
        let fresh = await model.sharedMailboxes(entry.id)
        asking = false
        // A switch or an add answered while this was asked is newer, so it stays. Otherwise only
        // a different answer redraws, so what is being typed on the page is left alone.
        if answer == before, fresh != answer { answer = fresh }
    }
}
