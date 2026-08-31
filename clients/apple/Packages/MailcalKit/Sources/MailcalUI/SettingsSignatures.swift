// The Signatures settings category (docs/signatures.md): the library, write once, reuse on any
// account, above the per-account defaults, one "For new messages" and one "For replies or
// forwards" picker each. State lives in Rust (`model.signatures` mirrors the snapshot); these
// render it and dispatch the setters, which re-signal `Surface::Settings`.
//
// Two things the layout is deliberate about. The library comes first because an account picker
// with nothing to pick is meaningless, a first-time user has to write a signature before the
// defaults mean anything. And "None" is a real option in both pickers rather than a separate
// enable switch: "None for both" already says "no signature on this account".

import MailcalBindings
import SwiftUI

/// The library: every signature the user has written, each editable, renameable and deletable,
/// plus the button that writes a new one.
struct SignatureLibraryView: View {
    var model: MailboxModel

    /// The signature being edited in the sheet, or `nil` when none is. A `nil` id means "a new
    /// signature", the sheet is the same either way, only its title and what Save does differ.
    @State private var editing: EditingSignature?
    @State private var deleting: SignatureRow?

    /// What the editor sheet was opened for. `id == nil` is a create.
    private struct EditingSignature: Identifiable {
        let id: String
        let signatureId: String?
        let name: String
        let bodyHTML: String?
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if model.signatures.signatures.isEmpty {
                Text(L10n.settings_signatures_empty())
                    .font(.callout)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(model.signatures.signatures, id: \.id) { signature in
                    row(signature)
                }
            }
            Button {
                editing = EditingSignature(
                    id: "new",
                    signatureId: nil,
                    name: L10n.settings_signatures_default_name(),
                    bodyHTML: nil
                )
            } label: {
                Label(L10n.settings_signatures_add(), systemImage: "plus")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
        }
        .sheet(item: $editing) { context in
            SignatureEditorView(
                title: context.signatureId == nil
                    ? L10n.settings_signatures_add()
                    : context.name,
                initialName: context.name,
                initialBodyHTML: context.bodyHTML,
                save: { name, html, plain in
                    if let id = context.signatureId {
                        model.updateSignature(id, name: name, bodyHTML: html, bodyPlain: plain)
                    } else {
                        model.createSignature(name: name, bodyHTML: html, bodyPlain: plain)
                    }
                    editing = nil
                },
                cancel: { editing = nil }
            )
        }
        // An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover, and a
        // popover DROPS the `.cancel`-role button, so this read as one destructive button with no
        // way out. See the remove-account alert in Mailcal.swift for the full note.
        .alert(
            L10n.settings_signatures_delete_title(),
            isPresented: deletingBinding
        ) {
            Button(L10n.settings_signatures_delete(), role: .destructive) {
                if let id = deleting?.id { model.deleteSignature(id) }
                deleting = nil
            }
            Button(L10n.action_cancel(), role: .cancel) { deleting = nil }
        } message: {
            Text(L10n.settings_signatures_delete_message())
        }
    }

    @ViewBuilder
    private func row(_ signature: SignatureRow) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "signature").foregroundStyle(.secondary)
            Text(signature.name).lineLimit(1).truncationMode(.middle)
            Spacer()
            // The whole row is not a button: delete sits next to it, and a stray click that opens
            // an editor is recoverable while one that deletes is not.
            Button(L10n.settings_signatures_edit()) {
                editing = EditingSignature(
                    id: signature.id,
                    signatureId: signature.id,
                    name: signature.name,
                    bodyHTML: model.signatureHTML(signature.id) ?? ""
                )
            }
            .buttonStyle(.borderless)
            Button(role: .destructive) {
                deleting = signature
            } label: {
                Image(systemName: "trash")
            }
            .buttonStyle(.borderless)
            .help(L10n.settings_signatures_delete())
        }
        .padding(.vertical, 2)
    }

    /// `confirmationDialog` wants a `Bool` binding, while the row that triggered it is what we
    /// need to delete, so the optional doubles as both.
    private var deletingBinding: Binding<Bool> {
        Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })
    }
}

/// The per-account defaults: for each configured account, which signature a new message opens with
/// and which a reply or forward does. Each independently, each with "None".
struct AccountSignatureDefaults: View {
    var model: MailboxModel

    var body: some View {
        if model.signatures.accounts.isEmpty {
            Text(L10n.settings_accounts_empty())
                .font(.callout)
                .foregroundStyle(.secondary)
        } else {
            VStack(alignment: .leading, spacing: 14) {
                ForEach(model.signatures.accounts, id: \.accountId) { account in
                    VStack(alignment: .leading, spacing: 6) {
                        // With one account the address is still shown: the setting is per account,
                        // and a user who later adds a second must not have to relearn that.
                        Text(account.email)
                            .font(.subheadline)
                            .bold()
                            .lineLimit(1)
                            .truncationMode(.middle)
                        slotPicker(
                            L10n.settings_signatures_new_message_label(),
                            account,
                            .newMessage,
                            account.newMessage
                        )
                        slotPicker(
                            L10n.settings_signatures_reply_forward_label(),
                            account,
                            .replyForward,
                            account.replyForward
                        )
                    }
                }
            }
        }
    }

    /// The label is drawn beside the Picker rather than left to it: outside a Form a `.menu`
    /// Picker drops its label on iOS, which would leave the two rows indistinguishable whenever
    /// both slots hold the same signature (the same reason `SwipeActionsPicker` does this).
    @ViewBuilder
    private func slotPicker(
        _ label: String,
        _ account: AccountSignatureRow,
        _ slot: SignatureSlotKind,
        _ selected: String?
    ) -> some View {
        HStack(spacing: 10) {
            Text(label).font(.callout).frame(width: 170, alignment: .leading)
            Picker(label, selection: binding(account, slot, selected)) {
                Text(L10n.settings_signatures_none()).tag(String?.none)
                ForEach(model.signatures.signatures, id: \.id) { signature in
                    Text(signature.name).tag(Optional(signature.id))
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .frame(maxWidth: 220, alignment: .leading)
        }
    }

    private func binding(
        _ account: AccountSignatureRow,
        _ slot: SignatureSlotKind,
        _ selected: String?
    ) -> Binding<String?> {
        Binding(
            get: { selected },
            set: { model.setAccountSignature(account.accountId, slot, $0) }
        )
    }
}
