//! Which signature each account uses, and the accessors that keep no assignment dangling
//! (`docs/signatures.md`, "No dangling assignment").

use super::Preferences;
use crate::signatures::{AccountSignatureAssignment, SignatureId, SignatureSlot};

impl Preferences {
    /// Which signature `account` uses in each slot: the empty assignment (no signature either
    /// way) for an account nobody has configured.
    #[must_use]
    pub fn account_signature(&self, account: &str) -> AccountSignatureAssignment {
        self.signature_assignments
            .get(account)
            .cloned()
            .unwrap_or_default()
    }

    /// Assigns (or clears, with `None`) one of `account`'s signature slots. An account left with
    /// nothing in either slot has its entry dropped rather than persisted as an empty table, so
    /// the file does not accumulate a row per account the user merely looked at.
    pub fn set_account_signature(
        &mut self,
        account: &str,
        slot: SignatureSlot,
        signature: Option<SignatureId>,
    ) {
        let entry = self
            .signature_assignments
            .entry(account.to_owned())
            .or_default();
        entry.set_slot(slot, signature);
        if entry.is_empty() {
            self.signature_assignments.remove(account);
        }
    }

    /// Drops every signature assignment for an account; used when the account is removed, so a
    /// later re-add starts with no signature rather than inheriting a pointer to one the user may
    /// meanwhile have deleted. Returns whether anything was stored for it.
    pub fn remove_account_signature(&mut self, account: &str) -> bool {
        self.signature_assignments.remove(account).is_some()
    }

    /// Clears `signature` from **every** account slot that points at it, and reports whether any
    /// did. Called when a signature is deleted from the library: an assignment naming a signature
    /// that no longer exists would silently mean "no signature", which is the same outcome but
    /// leaves a dangling id in the file to confuse the next reader.
    pub fn forget_signature(&mut self, signature: &SignatureId) -> bool {
        let mut cleared = false;
        for assignment in self.signature_assignments.values_mut() {
            if assignment.new_message.as_ref() == Some(signature) {
                assignment.new_message = None;
                cleared = true;
            }
            if assignment.reply_forward.as_ref() == Some(signature) {
                assignment.reply_forward = None;
                cleared = true;
            }
        }
        self.signature_assignments
            .retain(|_, assignment| !assignment.is_empty());
        cleared
    }
}
