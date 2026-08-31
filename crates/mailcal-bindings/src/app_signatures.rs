//! Signature-surface FFI methods on [`MailcalApp`]: the library's CRUD, the per-account
//! assignments, and the two resolutions a composer runs (the account's signature for its mode, and
//! a named one for the per-message override). Split out of `lib.rs` to keep each file under the
//! 500-line limit; the object is defined in `lib.rs`, and UniFFI collects these exported methods
//! crate-wide.
//!
//! The CRUD methods are direct methods rather than `Intent`s because they **return values** (a
//! create hands back the minted id, a delete reports whether the id named anything): the same
//! reason `add_account` is a method. See `docs/signatures.md`.

use crate::{MailcalApp, SignatureBody, SignatureRow, SignatureSlotKind, SignaturesSnapshot};

#[uniffi::export]
impl MailcalApp {
    /// The signatures surface (pulled after a `Surface::Settings` signal): the user's library in
    /// their chosen order, plus one row per configured account carrying its new-message and
    /// reply-forward assignments.
    ///
    /// Metadata only: a signature's body is fetched separately with
    /// [`MailcalApp::signature_html`], so opening Settings does not drag every embedded logo
    /// across the FFI to draw a list of names.
    pub fn signatures(&self) -> SignaturesSnapshot {
        self.runtime.block_on(self.app.signatures()).into()
    }

    /// One signature's HTML body, or `None` when the id names nothing; what a signature editor
    /// loads when the user opens an existing signature.
    #[must_use]
    pub fn signature_html(&self, id: String) -> Option<String> {
        self.app.signature_html(&id)
    }

    /// Creates a signature and returns its row, **including the minted id**, so a host can select
    /// what it just created without re-pulling the snapshot and guessing which row is new.
    /// Signals `Surface::Settings`.
    pub fn create_signature(
        &self,
        name: String,
        body_html: String,
        body_plain: String,
    ) -> SignatureRow {
        self.runtime
            .block_on(self.app.create_signature(name, body_html, body_plain))
            .into()
    }

    /// Replaces a signature's name and body. Returns whether the id named one: an unknown id is
    /// a no-op, never a silent create. Signals `Surface::Settings`.
    pub fn update_signature(
        &self,
        id: String,
        name: String,
        body_html: String,
        body_plain: String,
    ) -> bool {
        self.runtime
            .block_on(self.app.update_signature(&id, name, body_html, body_plain))
    }

    /// Deletes a signature and clears it from every account slot that pointed at it, so no
    /// assignment is left naming something that no longer exists. Returns whether the id named
    /// one. Signals `Surface::Settings`.
    pub fn delete_signature(&self, id: String) -> bool {
        self.runtime.block_on(self.app.delete_signature(&id))
    }

    /// Assigns (or clears, with `None`) which signature an account uses in one slot, then signals
    /// `Surface::Settings`. An id naming nothing in the library clears the slot instead of
    /// storing a pointer that resolves to nothing.
    pub fn set_account_signature(
        &self,
        account: String,
        slot: SignatureSlotKind,
        signature: Option<String>,
    ) {
        self.runtime.block_on(
            self.app
                .set_account_signature(&account, slot.into(), signature),
        );
    }

    /// The signature a composer should open with for `account` in `slot`; its id and both
    /// bodies, or `None` when that slot is unassigned. A host calls this when it opens a
    /// composer, and **again when the From account changes**, so the account's own signature
    /// follows the sender.
    #[must_use]
    pub fn resolve_signature(
        &self,
        account: String,
        slot: SignatureSlotKind,
    ) -> Option<SignatureBody> {
        self.app
            .resolve_signature(&account, slot.into())
            .map(Into::into)
    }

    /// One signature's id and both bodies by id: the composer's per-message override, where the
    /// user names a signature directly instead of inheriting the account's.
    #[must_use]
    pub fn signature_body(&self, id: String) -> Option<SignatureBody> {
        self.app.signature_body(&id).map(Into::into)
    }
}
