//! The token a pre-registration hands back, and the rollback a failed connect runs with it.

use super::AccountRegistry;
use crate::ConnectedAccount;

/// A registry entry written **before** its connect, holding whatever it displaced so a failed
/// connect can put the registry back exactly as it was.
///
/// The displaced entry matters more than it looks. `complete_microsoft_login` is *also* the
/// re-authentication path (the host re-runs sign-in for an account it already has) so an entry is
/// routinely there already, and a rollback that merely *removed* would delete a live account's
/// re-connection state because its re-auth failed.
pub(crate) struct Registered {
    pub(super) id: String,
    pub(super) replaced: Option<ConnectedAccount>,
}

impl Registered {
    /// Undoes the pre-registration after a failed connect: restores the entry that was displaced,
    /// or removes the one this added.
    pub(crate) fn rollback(self, registry: &AccountRegistry) {
        let mut entries = registry
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        match self.replaced {
            Some(previous) => entries.insert(self.id, previous),
            None => entries.remove(&self.id),
        };
    }

    /// Commits the registration by consuming the rollback token. The entry already contains the
    /// live token source prepared before the dial; committing must not replace any part of it.
    pub(crate) fn commit(self) {}
}
