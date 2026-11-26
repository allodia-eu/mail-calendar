//! Accounts whose own server is reached only over plaintext, so the account did not connect at
//! all: its address is `http://` off this device, or the server led there.
//!
//! Not an outage: waiting does not change it, and the server was not asked. Not a refused sign-in
//! either: no new password or sign-in clears it. So the account is neither badged unreachable nor
//! asked to sign in again; the host says why and what clears it (`docs/provider-oauth.md` rule
//! 12). A retry dials it again, once, in case the server or its address changed.

use engine_api::AccountId;
use mailcal_account::InsecureCause;

use crate::{App, protocol::Surface};

impl<P: engine_api::Provider> App<P> {
    /// Records that `account`'s server is reached only over plaintext, for `cause`. Takes away an
    /// outage badge an earlier dial left, which this answer replaces. Signals
    /// [`Surface::Connectivity`] only when that changes.
    pub fn note_insecure_server(&self, account: &AccountId, cause: InsecureCause) {
        let raised = self
            .use_problems
            .lock()
            .expect("use-problems mutex poisoned")
            .insecure
            .insert(account.as_str().to_owned(), cause)
            != Some(cause);
        self.set_account_reachable(account, true);
        if raised {
            log::warn!(
                "accounts: an account's server is reached only over plaintext; its sign-in is not \
                 sent there"
            );
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// Takes away what [`Self::note_insecure_server`] recorded: the account connected, or was
    /// removed. Signals [`Surface::Connectivity`] only when that changes.
    pub fn clear_insecure_server(&self, account: &AccountId) {
        let cleared = self
            .use_problems
            .lock()
            .expect("use-problems mutex poisoned")
            .insecure
            .remove(account.as_str())
            .is_some();
        if cleared {
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// The accounts whose own server is reached only over plaintext, and why, in id order.
    #[must_use]
    pub fn insecure_server_accounts(&self) -> Vec<(String, InsecureCause)> {
        self.use_problems
            .lock()
            .expect("use-problems mutex poisoned")
            .insecure
            .iter()
            .map(|(account, cause)| (account.clone(), *cause))
            .collect()
    }

    /// Accounts whose calendar or contacts are reached only over plaintext, for a retry to dial
    /// again: one dial each, in case the server or its address changed.
    #[must_use]
    pub fn accounts_with_insecure_use(&self) -> Vec<String> {
        [
            InsecureCause::PlaintextAddress,
            InsecureCause::LedToPlaintext,
        ]
        .into_iter()
        .flat_map(|cause| {
            self.accounts_with_use_problem(crate::UseProblem::Insecure(cause))
                .into_keys()
        })
        .collect()
    }
}

#[cfg(test)]
#[path = "insecure_servers_tests.rs"]
mod tests;
