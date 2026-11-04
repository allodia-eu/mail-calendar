//! Completing a Nextcloud sign-in on the surface that started it (`docs/nextcloud-sign-in.md`):
//! a new calendar and contacts account, the separate server of a mailbox, or an account signing
//! in again.
//!
//! Every completion keeps the grant usable when it fails, because the server hands an app
//! password out once: a connect that did not go through is tried again with the same grant, and a
//! grant that becomes no account is withdrawn with
//! [`MailcalApp::abandon_nextcloud_sign_in`](crate::MailcalApp::abandon_nextcloud_sign_in).

use engine_api::AccountId;
use mailcal_account::{
    AccountConfig, AccountSetup, Capabilities, Capability, ConnectionSecurity, SetupCredential,
};
use mailcal_nextcloud::Root;

use crate::{
    AccountCapability, AccountRow, MailcalApp, MailcalError,
    account_registry::{SignInConfig, nextcloud_issued},
    account_sign_in::Replacement,
    account_split::nothing_to_move,
    nextcloud::NextcloudGrant,
};

#[uniffi::export]
impl MailcalApp {
    /// Adds the calendar and contacts account a Nextcloud sign-in granted, used for `uses`
    /// (calendar, contacts or both), at the server's DAV endpoint: the completion of the found
    /// card, the manual form and the link step.
    ///
    /// **Blocking** (a connect); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::AlreadySetUp`] when this Nextcloud account is set up already,
    /// under whichever login name; the grant can then go on that account through
    /// [`MailcalApp::complete_account_nextcloud_sign_in`]. Otherwise the errors of
    /// [`MailcalApp::add_account`].
    pub fn add_nextcloud_account(
        &self,
        grant: String,
        uses: Vec<AccountCapability>,
    ) -> Result<AccountRow, MailcalError> {
        let grant = NextcloudGrant::parse(&grant)?;
        if let Some(existing) = self
            .registry
            .nextcloud_account(grant.root.as_str(), grant.user.as_deref())
        {
            return Err(MailcalError::AlreadySetUp {
                account_id: existing,
            });
        }
        let uses: Capabilities = uses.into_iter().map(Capability::from).collect();
        let dav = grant.root.dav_url();
        let setup = AccountSetup {
            imap_host: String::new(),
            username: grant.credentials.login_name.clone(),
            credential: SetupCredential::Nextcloud {
                app_password: grant.credentials.app_password.expose().to_owned(),
                login: grant.login(),
            },
            smtp_host: None,
            caldav_base_url: uses.contains(Capability::Calendar).then(|| dav.clone()),
            carddav_base_url: uses.contains(Capability::Contacts).then_some(dav),
            imap_security: ConnectionSecurity::ImplicitTls,
            smtp_security: ConnectionSecurity::ImplicitTls,
            accepted_certificate: None,
            uses: Some(uses),
        };
        let config_toml = mailcal_account::build_config_toml(&setup)
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        self.add_account(config_toml)
    }

    /// Moves `uses` (calendar, contacts or both) of the mailbox `account_id` to an account of
    /// their own that a Nextcloud sign-in at their server granted, and links the two: the
    /// separate-server step's completion when that server is Nextcloud
    /// (`docs/accounts.md` rule 18).
    ///
    /// Returns the new account's id. **Blocking** (two connects); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not a standards mailbox, `uses`
    /// naming no server it has, or a grant from another server than theirs; otherwise the errors
    /// of [`MailcalApp::add_linked_calendar_account`]. Both accounts are unchanged on every
    /// failure.
    pub fn add_linked_nextcloud_account(
        &self,
        account_id: String,
        uses: Vec<AccountCapability>,
        grant: String,
    ) -> Result<String, MailcalError> {
        let grant = NextcloudGrant::parse(&grant)?;
        let config = match self.switchable(&account_id)? {
            SignInConfig::Imap(config) if config.imap.is_some() => config,
            _ => {
                return Err(MailcalError::Config(
                    "only a standards mailbox keeps its calendar on a server of its own".to_owned(),
                ));
            }
        };
        let uses: Vec<Capability> = uses.into_iter().map(Capability::from).collect();
        let calendar = config
            .calendar_account_for(&uses)
            .ok_or_else(nothing_to_move)?;
        if server_of(&calendar).as_ref() != Some(&grant.root) {
            return Err(MailcalError::Config(
                "that sign-in was for another server".to_owned(),
            ));
        }
        let calendar = calendar.with_nextcloud_password(
            &grant.credentials.login_name,
            grant.credentials.app_password.expose(),
            &grant.root.dav_url(),
            grant.login(),
        );
        let moved_to = self.split_out(&account_id, calendar, *config, &uses)?;
        // Linked to an account already set up, which keeps the password it has: this one becomes
        // no account, so it leaves the server's list of devices now.
        let stored = self.registry.nextcloud_credentials(&moved_to);
        if stored.is_none_or(|(_, stored)| stored.app_password != grant.credentials.app_password) {
            self.revoke_later(
                grant.root,
                grant.credentials,
                "a sign-in for an account already set up",
            );
        }
        Ok(moved_to)
    }

    /// Puts the app password a Nextcloud sign-in granted on `account_id`, a calendar and contacts
    /// account at that server: signing it in again when its password was refused, or switching it
    /// from a typed one. It keeps its id, uses, links and settings; the app password it replaces,
    /// when a Nextcloud sign-in issued that one too, is withdrawn once the new one is stored.
    ///
    /// **Blocking** (a connect); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::DifferentAccount`] when the grant signs in as somebody else, whose
    /// app password is then withdrawn; [`MailcalError::Config`] for an account that is not a
    /// calendar and contacts account at that server; and [`MailcalError::SignInRefused`] or
    /// [`MailcalError::Connect`] when the new password does not connect. The account is unchanged
    /// on every failure.
    pub fn complete_account_nextcloud_sign_in(
        &self,
        account_id: String,
        grant: String,
    ) -> Result<(), MailcalError> {
        let grant = NextcloudGrant::parse(&grant)?;
        let config = match self.switchable(&account_id)? {
            SignInConfig::Imap(config) if config.imap.is_none() => config,
            _ => {
                return Err(MailcalError::Config(
                    "only a calendar and contacts account signs in with Nextcloud".to_owned(),
                ));
            }
        };
        if server_of(&config).as_ref() != Some(&grant.root) {
            return Err(MailcalError::Config(
                "that sign-in was for another server".to_owned(),
            ));
        }
        if !same_person(&config, &grant) {
            log::info!(
                "nextcloud: [{}] the sign-in was for somebody else",
                mailcal_account::account_log_handle(&account_id),
            );
            self.revoke_later(
                grant.root,
                grant.credentials,
                "a sign-in for another account",
            );
            return Err(MailcalError::DifferentAccount);
        }
        let replaced = nextcloud_issued(&config)
            .filter(|(_, old)| old.app_password != grant.credentials.app_password);
        let mut signed = config.with_nextcloud_password(
            &grant.credentials.login_name,
            grant.credentials.app_password.expose(),
            &grant.root.dav_url(),
            grant.login(),
        );
        // The id is derived from the login, which a sign-in may spell differently from the last.
        signed.shape.id = Some(
            AccountId::try_from(account_id.as_str())
                .map_err(|err| MailcalError::Config(err.to_string()))?,
        );
        self.install_credential(&account_id, Replacement::Imap(signed), &[])?;
        if let Some((root, old)) = replaced {
            self.revoke_later(root, old, "a replaced sign-in");
        }
        Ok(())
    }
}

/// The Nextcloud server `config`'s calendar, or its address book, is at.
pub(crate) fn server_of(config: &AccountConfig) -> Option<Root> {
    let base_url = config
        .caldav
        .as_ref()
        .map(|caldav| caldav.base_url.as_str())
        .or_else(|| {
            config
                .carddav
                .as_ref()
                .map(|carddav| carddav.base_url.as_str())
        })?;
    Root::of(&mailcal_account::normalize_caldav_base_url(base_url))
}

/// Whether `grant` signs in as the person `config` is: the same Nextcloud user when both name
/// one, else a login the account already knows them by.
fn same_person(config: &AccountConfig, grant: &NextcloudGrant) -> bool {
    let stored_user = config
        .nextcloud
        .as_ref()
        .and_then(|login| login.user.as_deref());
    if let (Some(before), Some(now)) = (stored_user, grant.user.as_deref()) {
        return before == now;
    }
    let known = config.username();
    [
        Some(grant.credentials.login_name.as_str()),
        grant.user.as_deref(),
        grant.email.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|name| name.eq_ignore_ascii_case(known))
}

#[cfg(test)]
#[path = "nextcloud_accounts_tests.rs"]
mod nextcloud_accounts_tests;
