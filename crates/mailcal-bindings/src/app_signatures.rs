//! The signatures on [`MailcalApp`] (`docs/signatures.md`): Settings → Signatures, what an
//! `Intent::Signature` does, the two resolutions a composer runs, and the rules an editor asks
//! before the person presses.

use std::sync::Arc;

use mailcal_app::SignatureChange;

use crate::{MailcalApp, SignatureBody, SignatureIntent, SignatureSlotKind, SignaturesPage};

#[path = "signatures_page.rs"]
mod signatures_page;

impl MailcalApp {
    /// Handles an `Intent::Signature`. Returns at once: a save or a deletion runs on a thread of
    /// its own, and an assignment is applied here and written behind.
    pub(crate) fn dispatch_signature(&self, intent: SignatureIntent) {
        let change = match intent {
            SignatureIntent::Save {
                id,
                name,
                body_html,
                body_plain,
            } => SignatureChange::Save {
                id,
                name,
                body_html,
                body_plain,
            },
            SignatureIntent::Delete { id } => SignatureChange::Delete { id },
            SignatureIntent::Assign {
                account,
                slot,
                signature,
            } => {
                self.app.assign_signature(&account, slot.into(), signature);
                return;
            }
        };
        if self.app.queue_signature_change(change) {
            let app = Arc::clone(&self.app);
            std::thread::spawn(move || app.run_signature_changes());
        }
    }
}

#[uniffi::export]
impl MailcalApp {
    /// Settings → Signatures: the library, each account used for mail with its two slots, and
    /// every control on the page (pulled after `Surface::Signatures` and `Surface::Settings`).
    /// Read from memory, so a UI thread may ask it.
    ///
    /// Metadata only: a signature's body is fetched separately with
    /// [`MailcalApp::signature_html`], so opening Settings does not drag every embedded logo
    /// across the FFI to draw a list of names.
    pub fn signatures_page(&self) -> SignaturesPage {
        signatures_page::page(self.app.signatures())
    }

    /// One signature's HTML body, or `None` when the id names nothing; what a signature editor
    /// loads when the user opens an existing signature.
    #[must_use]
    pub fn signature_html(&self, id: String) -> Option<String> {
        self.app.signature_html(&id)
    }

    /// The signature a composer should open with for `account` in `slot`; its id and both
    /// bodies, or `None` when that slot is unassigned. A host calls this when it opens a
    /// composer, and **again when the From account changes**, so the account's own signature
    /// follows the sender. Read from memory.
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

#[cfg(test)]
#[path = "app_signatures_tests.rs"]
mod tests;
