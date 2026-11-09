//! A Nextcloud sign-in for an account that exists: its calendar and contacts on a server of their
//! own, or the account itself signing in again (`docs/nextcloud-sign-in.md`).
//!
//! Each asks about the server the stored account names rather than one a client passes, as the
//! OAuth calls beside them do (`dav_oauth`): a client never handles the stored URL, whose path
//! can carry the person's address.

use mailcal_account::{AccountConfig, Capability};
use mailcal_nextcloud::Root;

use crate::{
    AccountCapability, MailcalApp, MailcalError, NextcloudSignInStart,
    account_registry::SignInConfig, account_split::nothing_to_move, nextcloud_accounts::dav_url_of,
};

impl MailcalApp {
    /// Whether the server holding `uses` of the mailbox `account_id` is a Nextcloud server, so
    /// its separate-server form offers "Sign in with Nextcloud" above the password field. The
    /// core asks it for the account's page.
    ///
    /// **Blocking** (a few requests of at most three seconds each). Never an error.
    pub(crate) fn separate_server_offers_nextcloud_sign_in(
        &self,
        account_id: String,
        uses: Vec<AccountCapability>,
    ) -> bool {
        self.separate_calendar(&account_id, uses)
            .ok()
            .and_then(|calendar| dav_url_of(&calendar))
            .is_some_and(|url| self.offers_for(&url))
    }
}

#[uniffi::export]
impl MailcalApp {
    /// Starts a Nextcloud sign-in at the server holding `uses` of the mailbox `account_id`,
    /// completed with
    /// [`add_linked_nextcloud_account`](MailcalApp::add_linked_nextcloud_account).
    ///
    /// **Blocking** (the server is located, then asked to start one); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not a standards mailbox or `uses`
    /// naming no server it has, and [`MailcalError::Connect`] when the server starts no sign-in.
    pub fn begin_separate_server_nextcloud_sign_in(
        &self,
        account_id: String,
        uses: Vec<AccountCapability>,
    ) -> Result<NextcloudSignInStart, MailcalError> {
        let root = self.separate_root(&account_id, uses)?;
        self.begin_at(&root)
    }

    /// Starts a Nextcloud sign-in for the calendar and contacts account `account_id`, at its own
    /// server: signing it in again, or switching it from a typed password. Completed with
    /// [`complete_account_nextcloud_sign_in`](MailcalApp::complete_account_nextcloud_sign_in).
    ///
    /// **Blocking** (the server is located, then asked to start one); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account that is not a calendar and contacts
    /// account, and [`MailcalError::Connect`] when its server starts no sign-in.
    pub fn begin_account_nextcloud_sign_in(
        &self,
        account_id: String,
    ) -> Result<NextcloudSignInStart, MailcalError> {
        let root = self
            .calendar_account(&account_id)
            .and_then(|config| dav_url_of(&config))
            .and_then(|url| self.root_for(&url))
            .ok_or_else(|| {
                MailcalError::Config(
                    "only a calendar and contacts account signs in with Nextcloud".to_owned(),
                )
            })?;
        self.begin_at(&root)
    }
}

impl MailcalApp {
    /// The server holding `uses` of the mailbox `account_id`.
    fn separate_root(
        &self,
        account_id: &str,
        uses: Vec<AccountCapability>,
    ) -> Result<Root, MailcalError> {
        dav_url_of(&self.separate_calendar(account_id, uses)?)
            .and_then(|url| self.root_for(&url))
            .ok_or_else(|| MailcalError::Config("not a Nextcloud server address".to_owned()))
    }

    /// The calendar and contacts account `uses` of the mailbox `account_id` would become.
    fn separate_calendar(
        &self,
        account_id: &str,
        uses: Vec<AccountCapability>,
    ) -> Result<AccountConfig, MailcalError> {
        let uses: Vec<Capability> = uses.into_iter().map(Capability::from).collect();
        self.split_parts(account_id, &uses)
            .map(|(_, calendar)| calendar)
    }

    /// The mailbox `account_id`, and the calendar and contacts account `uses` of it would become.
    pub(crate) fn split_parts(
        &self,
        account_id: &str,
        uses: &[Capability],
    ) -> Result<(AccountConfig, AccountConfig), MailcalError> {
        let config = match self.switchable(account_id)? {
            SignInConfig::Imap(config) if config.imap.is_some() => config,
            _ => {
                return Err(MailcalError::Config(
                    "only a standards mailbox keeps its calendar on a server of its own".to_owned(),
                ));
            }
        };
        let calendar = config
            .calendar_account_for(uses)
            .ok_or_else(nothing_to_move)?;
        Ok((*config, calendar))
    }

    /// The switch a calendar and contacts account's Settings page offers: signing in with
    /// Nextcloud again for one a Nextcloud sign-in made, or for the first time for one with a
    /// typed password whose server answers as Nextcloud. One that signs in at its server's own
    /// OAuth issuer offers nothing here, as before.
    pub(crate) fn calendar_sign_in_options(
        &self,
        config: &AccountConfig,
    ) -> crate::AccountSignInOptions {
        let nextcloud = if config.nextcloud.is_some() {
            true
        } else {
            !config.is_oauth() && dav_url_of(config).is_some_and(|url| self.offers_for(&url))
        };
        crate::AccountSignInOptions {
            nextcloud,
            ..crate::account_sign_in::no_options()
        }
    }

    /// `account_id`'s config, when it is a calendar and contacts account: the one kind a
    /// Nextcloud sign-in makes, or can sign in again.
    pub(crate) fn calendar_account(&self, account_id: &str) -> Option<AccountConfig> {
        match self.registry.sign_in_config(account_id)? {
            SignInConfig::Imap(config) if config.imap.is_none() => Some(*config),
            _ => None,
        }
    }
}
