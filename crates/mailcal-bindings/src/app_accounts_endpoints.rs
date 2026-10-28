//! Editing an account's servers and sign-in from Settings → Accounts.

use mailcal_account::CredentialOrigin;

use crate::{
    AccountEndpoints, MailcalApp, MailcalError, account_repair::CredentialPersistence,
    app_accounts_uses::domain_of, boot,
};

#[uniffi::export]
impl MailcalApp {
    /// Replaces `account_id`'s servers and login with `endpoints`, after proving they connect.
    ///
    /// The account keeps its id, its uses, its links, its settings and its signatures. A use
    /// whose server moved to another host has what the device holds of it deleted and synced
    /// again from the new one; a new port, security, login or password keeps it. Start from the
    /// entry's [`AccountEntry::endpoints`](crate::AccountEntry::endpoints) and leave `password`
    /// `None` to keep the stored one.
    ///
    /// **Blocking** (provider connect); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an unknown account, one that signs in through its
    /// provider, a use left without its server, an empty login or password, or servers that
    /// belong to another account already set up. Returns [`MailcalError::SignInRefused`] when the
    /// new servers refuse the login, [`MailcalError::Connect`] (or
    /// [`MailcalError::CertificateRejected`]) when they do not connect, and
    /// [`MailcalError::Connect`] when the host's store refuses them. On every failure the account
    /// is left exactly as it was.
    pub fn update_account_endpoints(
        &self,
        account_id: String,
        endpoints: AccountEndpoints,
    ) -> Result<(), MailcalError> {
        self.replace_endpoints(account_id, endpoints, &[])
    }
}

impl MailcalApp {
    /// [`MailcalApp::update_account_endpoints`], refusing the edit unless each use in
    /// `must_connect` connects on the new servers, with
    /// [`MailcalError::UseNotConnected`] naming the first that did not.
    pub(crate) fn replace_endpoints(
        &self,
        account_id: String,
        endpoints: AccountEndpoints,
        must_connect: &[mailcal_account::Capability],
    ) -> Result<(), MailcalError> {
        let (config_toml, moved) = self
            .registry
            .edited_endpoints(&account_id, &endpoints.into())?;
        let sink = crate::token_sink::token_sink(&self.registry, &self.credential_store);
        let prepared =
            boot::prepare_stored_account(&config_toml, &sink, CredentialOrigin::FreshSignIn)?;
        let forget: Vec<_> = moved.iter().map(domain_of).collect();
        let shared_before = self.registry.shared_mailboxes(&account_id);
        // The principals the old servers named would start the new config's connects at the
        // old servers, so they go first; a failed replacement only costs the next connect its
        // shortcut.
        self.registry.forget_dav_starts(&account_id);
        self.install_replacement(
            &account_id,
            prepared,
            CredentialPersistence::Provided(config_toml),
            "edited servers",
            &forget,
            None,
            must_connect,
        )?;
        // A mail server on another host leaves no shared mailbox the old one opened.
        let shared_after = self.registry.shared_mailboxes(&account_id);
        for (child, _) in shared_before {
            if !shared_after.iter().any(|(kept, _)| *kept == child) {
                self.drop_shared_mailbox(&child);
            }
        }
        self.app.accounts_changed();
        Ok(())
    }
}

#[cfg(test)]
#[path = "app_accounts_endpoints_tests.rs"]
mod tests;
