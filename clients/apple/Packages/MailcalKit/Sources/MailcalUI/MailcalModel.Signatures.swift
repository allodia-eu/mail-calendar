// The signature library's actions on the model: the CRUD the Settings screen drives, and the two
// resolutions the composer runs (the account's signature for the mode it opened in, and a named
// one for the per-message override). State lives in Rust, `model.signatures` mirrors the snapshot
// and refreshes on every `Surface::Settings` signal; these dispatch the setters.
//
// The mode → slot rule is the one piece of signature logic a client owns, so it is a plain
// function here rather than a knot of view state, it is what the JVM/unit suites can pin.

import Foundation
import MailcalBindings

/// Which signature slot a composer opened in `mode` should seed from. A reply, a reply-all and a
/// forward share one slot (Outlook's grouping): all three continue an existing message, and
/// splitting them makes a setting nobody sets.
///
/// A free function rather than a method on the `@MainActor` model, so the rule can be tested
/// without an actor hop, it is pure, and it is the one piece of signature logic the client owns
/// (`SignatureSlotTests`). Same shape as `messageAfterRemoving`.
func signatureSlot(for mode: RichComposeMode) -> SignatureSlotKind {
    mode == .new ? .newMessage : .replyForward
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

    /// Creates a signature and returns its row, the minted id included, so the caller can select
    /// what it just made without re-pulling and guessing which row is new.
    @discardableResult
    func createSignature(name: String, bodyHTML: String, bodyPlain: String) -> SignatureRow? {
        app?.createSignature(name: name, bodyHtml: bodyHTML, bodyPlain: bodyPlain)
    }

    /// Replaces a signature's name and body. An id that names nothing is a no-op.
    func updateSignature(_ id: String, name: String, bodyHTML: String, bodyPlain: String) {
        _ = app?.updateSignature(id: id, name: name, bodyHtml: bodyHTML, bodyPlain: bodyPlain)
    }

    /// Deletes a signature, clearing it from every account that used it.
    func deleteSignature(_ id: String) {
        _ = app?.deleteSignature(id: id)
    }

    /// Assigns (or clears, with `nil`) which signature an account uses in one slot.
    func setAccountSignature(_ account: String, _ slot: SignatureSlotKind, _ signature: String?) {
        app?.setAccountSignature(account: account, slot: slot, signature: signature)
    }
}
