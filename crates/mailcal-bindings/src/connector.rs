//! The host-side on-demand folder connector: the bindings' implementation of
//! `mailcal_app::MailboxConnector`.
//!
//! When the app needs a folder it hasn't synced (a custom/untagged folder the eager bind skipped),
//! it asks the registry how to dial that account and opens a provider bound to the one folder. A
//! failure (unknown account, login error) yields `None`, so the app leaves the folder empty rather
//! than failing navigation.
//!
//! Everything family-specific lives in [`AccountDial`](crate::account_registry::AccountDial). This
//! module used to hold a private four-variant enum cloned out of the registry by hand, which was
//! one of the five places that each independently knew how to rebuild an account.

use async_trait::async_trait;
use engine_api::{AccountId, Provider};
use mailcal_account::SyncDepth;
use mailcal_app::MailboxConnector;

use crate::SharedRegistry;

/// Connects a provider for any folder of a registered account, on demand.
pub(crate) struct HostConnector {
    /// The account registry, shared with [`MailcalApp`](crate::MailcalApp).
    pub(crate) registry: SharedRegistry,
}

#[async_trait]
impl MailboxConnector<Box<dyn Provider>> for HostConnector {
    async fn connect_folder(
        &self,
        account: &AccountId,
        mailbox_key: &str,
        _depth: SyncDepth,
    ) -> Option<Box<dyn Provider>> {
        // The dial is a snapshot, so no registry lock is held across the connect below.
        self.registry
            .dial(account.as_str())?
            .connect_folder(mailbox_key)
            .await
    }
}
