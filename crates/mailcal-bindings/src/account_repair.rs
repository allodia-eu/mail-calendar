//! Replacing a dead credential in place without removing the account or its persisted mail.

use std::sync::Arc;

use mailcal_account::CredentialOrigin;

use crate::{MailcalApp, MailcalError, boot, connection_log};

/// Which bytes a successful repair writes. OAuth can rotate during its validating dial, so its
/// durable value comes back out of the registry; a password has no rotation and uses the candidate
/// config assembled from the existing endpoints plus the newly entered secret.
pub(crate) enum CredentialPersistence {
    Provided(String),
    RegisteredGrant,
}

#[uniffi::export]
impl MailcalApp {
    /// Replaces the password or pasted JMAP secret of `account_id`, after proving it connects.
    /// The account keeps its identity, settings, cached mail, and switcher position; the core
    /// writes the accepted credential through its
    /// [`AccountCredentialStore`](crate::AccountCredentialStore) and runs a catch-up refresh.
    ///
    /// **Blocking** (provider connect); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an unknown or OAuth account, an empty secret, or a
    /// replacement that would change the account id. Returns [`MailcalError::Connect`] when the
    /// server refuses the candidate or the secure store cannot persist it. The existing registry
    /// entry and stored credential remain unchanged on every failure.
    pub fn replace_account_secret(
        &self,
        account_id: String,
        secret: String,
    ) -> Result<(), MailcalError> {
        let config_toml = self
            .registry
            .replacement_secret_toml(&account_id, &secret)
            .map_err(MailcalError::Config)?;
        let sink = crate::token_sink::token_sink(&self.registry, &self.credential_store);
        let prepared =
            boot::prepare_stored_account(&config_toml, &sink, CredentialOrigin::FreshSignIn)?;
        self.install_repaired_account(
            &account_id,
            prepared,
            CredentialPersistence::Provided(config_toml),
            "stored credential",
        )
    }
}

impl MailcalApp {
    /// Validates, persists and installs a prepared replacement, shared by browser JMAP re-auth and
    /// password/API-token repair so the two cannot disagree on rollback or account preservation.
    pub(crate) fn install_repaired_account(
        &self,
        account_id: &str,
        prepared: boot::PreparedAccount,
        persistence: CredentialPersistence,
        family: &'static str,
    ) -> Result<(), MailcalError> {
        let id = prepared.account.id.clone();
        if id.as_str() != account_id {
            return Err(MailcalError::Config(
                "the replacement credential belongs to a different account".to_owned(),
            ));
        }
        let registered = self
            .registry
            .pre_register(account_id.to_owned(), prepared.connected);
        let Some(dial) = self.registry.dial(account_id) else {
            registered.rollback(&self.registry);
            return Err(MailcalError::Config(
                "the account could not be registered before connecting".to_owned(),
            ));
        };
        let outcome = match self
            .runtime
            .block_on(dial.run(&id, self.device_zone.clone()))
        {
            Ok(outcome) => outcome,
            Err(error) => {
                registered.rollback(&self.registry);
                return Err(MailcalError::Connect(error.to_string()));
            }
        };
        let persisted = match persistence {
            CredentialPersistence::Provided(config_toml) => {
                self.persist_credential(account_id, config_toml)
            }
            CredentialPersistence::RegisteredGrant => self.persist_registered_grant(account_id),
        };
        if let Err(error) = persisted {
            registered.rollback(&self.registry);
            return Err(error);
        }
        registered.commit();
        if let Some(error) = outcome.calendar_error {
            self.calendar_connect_errors
                .lock()
                .expect("calendar-errors mutex poisoned")
                .push(error);
        }
        self.refresh_analytics_accounts();
        connection_log::log_account_connection_info("reauth", family, &outcome.account);
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(async move { app.add_account_deferred(outcome.account).await });
        self.app.clear_signin_expired(&id);
        self.disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .remove(account_id);
        self.refresh_background(account_id);
        let app_sync = Arc::clone(&self.app);
        self.runtime
            .spawn(async move { app_sync.refresh_reconnected_account(&id).await });
        log::info!("credential repair: {family} account connected and stored; catch-up running");
        Ok(())
    }
}
