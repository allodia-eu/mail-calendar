// The rich composer's signature resolution and picker: the account's signature by default,
// overridable for this one message. Split out of RichComposerView.swift to keep it under 500
// lines.

import Foundation
import MailcalBindings
import SwiftUI

extension RichComposeView {
    /// Whether to show the signature picker: the feature is wired **and** the user has written at
    /// least one signature. With an empty library the picker would offer only "None", which is a
    /// control that cannot do anything.
    var showsSignaturePicker: Bool {
        !(signatures?.library.isEmpty ?? true)
    }

    /// The signature currently on this message: the user's explicit choice if they made one, else
    /// whatever the From account assigns for this mode.
    private var effectiveSignature: SignatureBody? {
        switch signatureChoice {
        case nil: return accountSignature
        case .noSignature: return nil
        case let .signature(id): return signatures?.byId(id)
        }
    }

    /// The From account's signature for this compose mode, ignoring any per-message override.
    private var accountSignature: SignatureBody? {
        guard let signatures, let account = resolvedFrom else { return nil }
        return signatures.forAccount(account, signatureSlot(for: mode))
    }

    /// The id the picker shows as selected, `nil` is the "None" row.
    private var signatureBinding: Binding<String?> {
        Binding(
            get: { effectiveSignature?.id },
            set: { id in
                signatureChoice = id.map(SignatureChoice.signature) ?? .noSignature
                editor.setSignature(effectiveSignature.flatMap(Self.signatureSeed))
            }
        )
    }

    /// Re-seeds the signature after the From account changed. A no-op once the user has picked one
    /// explicitly: they chose it for this message, and swapping it under them would undo that.
    func fromAccountChanged() {
        guard signatureChoice == nil else { return }
        editor.setSignature(accountSignature.flatMap(Self.signatureSeed))
    }

    /// The `setComposerSignature` payload: the shape the Rust composer's `Block::Signature`
    /// round-trips, so what the editor hands back on submit is what the core already understands.
    ///
    /// Not `private`: RichComposerView.swift's `init` calls it too.
    static func signatureSeed(_ body: SignatureBody) -> String? {
        let payload: [String: Any] = ["body_html": body.bodyHtml, "body_plain": body.bodyPlain]
        guard let data = try? JSONSerialization.data(withJSONObject: payload) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// The signature control: one bar button that opens the library. The current choice is shown as
    /// a checkmark inside the menu rather than on the button, so the bar reads as a row of verbs:
    /// a button labelled with whichever signature happens to be selected would be a different width
    /// every time and would say nothing about what pressing it does.
    var signatureMenu: some View {
        Menu {
            // `.inline`, not the default: a labelled Picker inside a Menu renders as a *submenu*,
            // so choosing a signature would cost two clicks and a hover. Inline puts the options
            // straight in the menu, with the current one checked.
            Picker(L10n.compose_signature_label(), selection: signatureBinding) {
                Text(L10n.settings_signatures_none()).tag(String?.none)
                ForEach(signatures?.library ?? [], id: \.id) { signature in
                    Text(signature.name).tag(Optional(signature.id))
                }
            }
            .pickerStyle(.inline)
        } label: {
            Label(L10n.compose_signature_label(), systemImage: "signature")
        }
        // Styled as a button, not a borderless menu: it sits next to Attach files, and a plain
        // blue label beside a bordered capsule reads as two different kinds of control rather
        // than two items in one bar (most visible on iOS).
        .menuStyle(.button)
        .buttonStyle(.bordered)
        .controlSize(.small)
        .fixedSize()
    }
}
