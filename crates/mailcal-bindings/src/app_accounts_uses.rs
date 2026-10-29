//! Switching what an account is used for, from Settings → Accounts.

use std::sync::Arc;

use engine_api::{AccountId, SearchDomain};
use mailcal_account::Capability;

use crate::{
    AccountCapability, CapabilityChange, MailcalApp, MailcalError, account_registry::UseChange,
};

#[uniffi::export]
impl MailcalApp {
    /// Switches `capability` on or off for `account_id`, stores the choice, and reconnects the
    /// account to open what it is now used for.
    ///
    /// Switching a use off stops it and deletes what this device holds of it at once; the
    /// provider's permission is left as it is, because an app cannot narrow it. Switching
    /// contacts off switches colleagues off too. Switching a use on needs nothing more when the
    /// account can already open it; otherwise nothing changes and the answer says what is
    /// missing: the provider's permission ([`CapabilityChange::NeedsConsent`]) or a server
    /// ([`CapabilityChange::NeedsEndpoint`]).
    ///
    /// **Blocking** (it deletes local data); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an unknown account, a use its kind does not offer,
    /// colleagues without contacts, or switching off the last of mail, calendar and contacts:
    /// such an account is removed instead. Returns [`MailcalError::Connect`] when the host's
    /// store refused the change, which is then not made.
    pub fn set_account_capability(
        &self,
        account_id: String,
        capability: AccountCapability,
        on: bool,
    ) -> Result<CapabilityChange, MailcalError> {
        let capability = Capability::from(capability);
        let (config_toml, previous) = match self.registry.set_use(&account_id, capability, on)? {
            UseChange::Unchanged => return Ok(CapabilityChange::Applied),
            UseChange::NeedsConsent => return Ok(CapabilityChange::NeedsConsent),
            UseChange::NeedsEndpoint => return Ok(CapabilityChange::NeedsEndpoint),
            UseChange::Changed {
                config_toml,
                previous,
            } => (config_toml, previous),
        };
        if let Err(err) = self.persist_config(&account_id, config_toml) {
            self.registry.restore_uses(&account_id, previous);
            return Err(err);
        }
        let id = AccountId::try_from(account_id.as_str())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        log::info!(
            "accounts: [{}] {} switched {}",
            mailcal_account::account_log_handle(&account_id),
            capability.as_str(),
            if on { "on" } else { "off" },
        );
        // Switching shared mailboxes off leaves the account's own mail as it is, so it is neither
        // listed without providers nor dialled again.
        let redial = on || capability != Capability::SharedMailboxes;
        // Nothing may sync into a domain while it is forgotten, so the account is listed without
        // providers first and dialled again only afterwards.
        if redial {
            self.list_without_providers(&id);
        }
        if !on && matches!(capability, Capability::Calendar | Capability::Contacts) {
            self.app
                .forget_use_problem(&id, capability == Capability::Calendar);
        }
        // The shared mailboxes are opened by this use, so they go with it; the account's own
        // mail stays unless mail itself was switched off. A refused write is reported once the
        // account is dialled again, so it is not left listed without providers.
        let dropped = if !on && matches!(capability, Capability::Mail | Capability::SharedMailboxes)
        {
            self.drop_shared_mailboxes(&account_id)
        } else {
            Ok(())
        };
        if !on && capability != Capability::SharedMailboxes {
            let app = Arc::clone(&self.app);
            let forgotten = id.clone();
            self.runtime.block_on(|| async move {
                app.forget_account_domain(&forgotten, domain_of(capability))
                    .await;
            });
        }
        if redial {
            self.disconnected
                .lock()
                .expect("disconnected mutex poisoned")
                .insert(account_id);
            self.retry_connections();
        }
        self.refresh_analytics_accounts();
        self.app.accounts_changed();
        dropped.map(|()| CapabilityChange::Applied)
    }
}

impl MailcalApp {
    /// Lists `id` again without providers and stops its background sync, so nothing it no longer
    /// opens keeps syncing until its next dial.
    pub(crate) fn list_without_providers(&self, id: &AccountId) {
        let Some(placeholder) = self.registry.placeholder(id) else {
            return;
        };
        self.background.apply(id.as_str(), None);
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(|| async move { app.add_account_deferred(placeholder).await });
    }
}

/// What the store forgets when an account stops being used for `capability`. Colleagues are
/// stored as cards beside the account's own with no line between them, so both go, and the
/// account's own come back with its next contacts sync. Shared mailboxes are accounts of their
/// own and are forgotten as such (`MailcalApp::drop_shared_mailboxes`).
pub(crate) const fn domain_of(capability: Capability) -> SearchDomain {
    match capability {
        Capability::Mail | Capability::SharedMailboxes => SearchDomain::Mail,
        Capability::Calendar => SearchDomain::Calendar,
        Capability::Contacts | Capability::Colleagues => SearchDomain::Contacts,
    }
}

#[cfg(test)]
#[path = "app_accounts_uses_tests.rs"]
mod tests;
