// The Signatures settings category (docs/signatures.md): the library, written once and used on
// any account, above the per-account defaults, one "For new messages" and one "For replies or
// forwards" picker each. Drawn from the core's page (`model.signatures`, `signatures_page`), every
// control through the control components; a change is an `Intent::Signature`.
//
// Two things the layout is deliberate about. The library comes first because an account picker
// with nothing to pick is meaningless, a first-time user has to write a signature before the
// defaults mean anything. And "None" is a real option in both pickers rather than a separate
// enable switch: "None for both" already says "no signature on this account".

import MailcalBindings
import SwiftUI

/// The library: every signature the user has written, each editable and deletable, plus the
/// button that writes a new one.
struct SignatureLibraryView: View {
    var model: MailboxModel

    /// The signature being edited in the sheet, or `nil` when none is.
    @State private var editing: EditingSignature?
    @State private var deleting: SignatureEntry?

    /// What the editor sheet was opened on: a signature in the library, or a new one under the
    /// page's `newId`.
    private struct EditingSignature: Identifiable {
        let session: SignatureEditorSession
        let isNew: Bool
        let name: String
        let bodyHTML: String?

        var id: String { session.id }
    }

    private var page: SignaturesPage { model.signatures }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if page.signatures.isEmpty {
                Text(L10n.settings_signatures_empty())
                    .font(.callout)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(page.signatures, id: \.id) { signature in
                    row(signature)
                }
            }
            ControlButton(
                title: L10n.settings_signatures_add(), systemImage: "plus", control: page.add
            ) {
                editing = EditingSignature(
                    session: SignatureEditorSession(id: page.newId, page: page),
                    isNew: true,
                    name: L10n.settings_signatures_default_name(),
                    bodyHTML: nil
                )
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
        }
        .sheet(item: $editing) { context in
            SignatureEditorView(
                model: model,
                session: context.session,
                title: context.isNew ? L10n.settings_signatures_add() : context.name,
                initialName: context.name,
                initialBodyHTML: context.bodyHTML,
                close: { editing = nil }
            )
        }
        .confirmAlert(
            $deleting,
            action: L10n.settings_signatures_delete(),
            title: { _ in L10n.settings_signatures_delete_title() },
            message: { _ in L10n.settings_signatures_delete_message() },
            confirm: { model.deleteSignature($0.id) }
        )
    }

    @ViewBuilder
    private func row(_ signature: SignatureEntry) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "signature").foregroundStyle(.secondary)
            Text(signature.name).lineLimit(1).truncationMode(.middle)
            Spacer()
            // The whole row is not a button: delete sits next to it, and a stray click that opens
            // an editor is recoverable while one that deletes is not.
            ControlButton(title: L10n.settings_signatures_edit(), control: signature.edit) {
                editing = EditingSignature(
                    session: SignatureEditorSession(id: signature.id, page: page),
                    isNew: false,
                    name: signature.name,
                    bodyHTML: model.signatureHTML(signature.id) ?? ""
                )
            }
            .buttonStyle(.borderless)
            ControlButton(
                title: L10n.settings_signatures_delete(), systemImage: "trash", role: .destructive,
                control: signature.delete
            ) {
                deleting = signature
            }
            .buttonStyle(.borderless)
            .labelStyle(.iconOnly)
            .help(L10n.settings_signatures_delete())
        }
        .padding(.vertical, 2)
    }
}

/// The per-account defaults: for each account used for mail, which signature a new message opens
/// with and which a reply or forward does. Each independently, each with "None".
struct AccountSignatureDefaults: View {
    var model: MailboxModel

    private var page: SignaturesPage { model.signatures }

    var body: some View {
        if page.accounts.isEmpty {
            Text(L10n.settings_accounts_empty())
                .font(.callout)
                .foregroundStyle(.secondary)
        } else {
            VStack(alignment: .leading, spacing: 14) {
                ForEach(page.accounts, id: \.accountId) { account in
                    VStack(alignment: .leading, spacing: 6) {
                        // With one account the address is still shown: the setting is per account,
                        // and a user who later adds a second must not have to relearn that.
                        Text(account.email)
                            .font(.headline)
                            .lineLimit(1)
                            .truncationMode(.middle)
                        slotPicker(
                            L10n.settings_signatures_new_message_label(),
                            account.accountId,
                            .newMessage,
                            account.newMessage
                        )
                        slotPicker(
                            L10n.settings_signatures_reply_forward_label(),
                            account.accountId,
                            .replyForward,
                            account.replyForward
                        )
                    }
                }
            }
        }
    }

    private func slotPicker(
        _ title: String,
        _ account: String,
        _ slot: SignatureSlotKind,
        _ picker: SignaturePicker
    ) -> some View {
        let library = page.signatures
        return ControlPicker(
            title: title,
            labels: SignatureSlotChoices.labels(library),
            selected: SignatureSlotChoices.position(of: picker.selected, in: library),
            control: picker.control
        ) { position in
            model.assignSignature(
                account, slot, SignatureSlotChoices.signature(at: position, in: library))
        }
    }
}
