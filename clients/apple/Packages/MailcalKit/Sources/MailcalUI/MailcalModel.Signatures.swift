// The signature library on the model (docs/signatures.md): the changes Settings → Signatures
// dispatches, and the two resolutions the composer runs (the account's signature for the mode it
// opened in, and a named one for the per-message override). `model.signatures` is the core's
// `signatures_page`, read again on every `Surface::Signatures` and `Surface::Settings` signal.
//
// The mode → slot rule is the one piece of signature logic a client owns, so it is a plain
// function here rather than a knot of view state, which the unit suite can pin.

import Foundation
import MailcalBindings

/// Which signature slot a composer opened in `mode` should seed from. A reply, a reply-all and a
/// forward share one slot (Outlook's grouping): all three continue an existing message, and
/// splitting them makes a setting nobody sets.
///
/// A free function rather than a method on the `@MainActor` model, so the rule can be tested
/// without an actor hop (`SignatureSlotTests`).
func signatureSlot(for mode: RichComposeMode) -> SignatureSlotKind {
    mode == .new ? .newMessage : .replyForward
}

extension SignaturesPage {
    /// The page before the core has opened: nothing listed and nothing to press.
    static var empty: SignaturesPage {
        SignaturesPage(
            signatures: [], accounts: [], add: .free(false), newId: "", save: .free(false),
            outcome: nil)
    }
}

extension MailboxModel {
    /// The signature `account` uses in `slot`, or `nil` when that slot is unassigned, which is how
    /// a user turns signatures off. Re-run when the composer's From account changes, so the
    /// signature follows the sender.
    func accountSignature(_ account: String, _ slot: SignatureSlotKind) -> SignatureBody? {
        app?.resolveSignature(account: account, slot: slot)
    }

    /// One signature by id, the composer's per-message override, where the user names a signature
    /// directly instead of inheriting the account's.
    func signatureBody(_ id: String) -> SignatureBody? {
        app?.signatureBody(id: id)
    }

    /// One signature's HTML body, for the Settings editor to load an existing signature into.
    func signatureHTML(_ id: String) -> String? {
        app?.signatureHtml(id: id)
    }

    /// Stores a signature under `id`: one in the library, or a new one under the page's `newId`.
    /// Returns at once; what it came to is the page's `outcome`.
    func saveSignature(_ id: String, name: String, bodyHTML: String, bodyPlain: String) {
        app?.dispatch(intent: .signature(
            intent: .save(id: id, name: name, bodyHtml: bodyHTML, bodyPlain: bodyPlain)))
    }

    /// Deletes a signature, clearing it from every account that used it. Returns at once.
    func deleteSignature(_ id: String) {
        app?.dispatch(intent: .signature(intent: .delete(id: id)))
    }

    /// Sets which signature an account uses in one slot, or none with `nil`.
    func assignSignature(_ account: String, _ slot: SignatureSlotKind, _ signature: String?) {
        app?.dispatch(intent: .signature(
            intent: .assign(account: account, slot: slot, signature: signature)))
    }
}
