//! Switching an account between a password and signing in through its provider, and signing a
//! signed-in account in again (`docs/accounts.md` rule 17).
//!
//! The account keeps its id, uses, links, settings and mail: only the credential changes, and
//! the new one connects before it is stored. Every route ends in
//! [`MailcalApp::install_credential`], which is the replacement a repaired password goes through,
//! so a switch that fails leaves the account exactly as it was.

use engine_api::addresses_match;
use mailcal_account::{
    AccountConfig, CredentialOrigin, ImapAuth, ImapAuthQuery, JmapAccountConfig, Secret,
    SetupRecommendation,
};
use serde::{Deserialize, Serialize};

use crate::{
    ImapLoginRequest, MailcalApp, MailcalError, account_registry::SignInConfig,
    account_repair::CredentialPersistence, boot,
};

/// Which ways in an account's server accepts, for the switch its Settings page offers.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountSignInOptions {
    /// Whether the account can sign in through its provider: switched to from a password, or
    /// signed in again when it already does.
    pub sign_in: bool,
    /// The provider's name for that button, when this build holds a registration naming one.
    pub provider_label: Option<String>,
    /// Whether the account can sign in with a password: kept, or switched to from signing in.
    pub password: bool,
}

/// The browser half of signing an existing account in: the URL to open in the platform auth
/// session, and the handle to pass back to [`MailcalApp::complete_account_sign_in`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct AccountSignInStart {
    /// The authorization URL to open.
    pub authorization_url: String,
    /// Opaque; hand it back unchanged.
    pub pending: String,
}

/// The pending sign-in of the kind the account is, round-tripped through the host.
#[derive(Serialize, Deserialize)]
enum PendingSignIn {
    Imap(String),
    Jmap(String),
}

/// An account's config with its new credential, before it has connected. Unboxed, as
/// `ConnectedAccount` is: there is one per switch, and it is moved straight into the dial.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Replacement {
    Imap(AccountConfig),
    Jmap(JmapAccountConfig),
}

#[uniffi::export]
impl MailcalApp {
    /// Which ways in `account_id`'s server accepts: signing in through the provider where setup
    /// would offer it, and a password where the server names a password mechanism (for JMAP,
    /// where setup offers its secret field, which is everywhere). Nothing for an unknown account
    /// and for Microsoft and Google, which sign in again through their own consent.
    ///
    /// **Blocking** (detection, a capability probe and issuer discovery, each bounded); call it
    /// off the UI thread and draw the switch once it answers. Never fails: an unanswered
    /// question offers nothing.
    pub fn account_sign_in_options(&self, account_id: String) -> AccountSignInOptions {
        match self.registry.sign_in_config(&account_id) {
            Some(SignInConfig::Imap(config)) => self.imap_sign_in_options(&account_id, &config),
            Some(SignInConfig::Jmap(config)) => AccountSignInOptions {
                sign_in: config.is_oauth()
                    || self
                        .jmap_oauth_available(config.email.clone(), Some(config.base_url.clone())),
                provider_label: None,
                password: true,
            },
            None => no_options(),
        }
    }

    /// Starts signing `account_id` in through its provider: again, from its stored grant and with
    /// no discovery, when it already signs in that way; for the first time, discovered as setup
    /// discovers it, when it signs in with a password. The browser targets the account's own
    /// address.
    ///
    /// **Blocking** (discovery and registration for a password account); call it off the UI
    /// thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an unknown account, one Settings cannot switch, or a
    /// server that offers no sign-in, and [`MailcalError::Connect`] when discovery fails.
    pub fn begin_account_sign_in(
        &self,
        account_id: String,
        redirect_uri: String,
    ) -> Result<AccountSignInStart, MailcalError> {
        let (pending, authorization_url) = match self.switchable(&account_id)? {
            SignInConfig::Imap(config) => {
                let start = match &config.oauth {
                    Some(grant) => self.sign_in_again(&config, grant)?,
                    None => {
                        let request = self.imap_login_request(&account_id, &config)?;
                        self.begin_imap_login(request, redirect_uri)?
                    }
                };
                (PendingSignIn::Imap(start.pending), start.authorization_url)
            }
            SignInConfig::Jmap(config) => {
                let start = if config.is_oauth() {
                    self.begin_jmap_reauth(account_id)?
                } else {
                    self.begin_jmap_login(
                        config.email.clone(),
                        Some(config.base_url.clone()),
                        redirect_uri,
                    )?
                };
                (PendingSignIn::Jmap(start.pending), start.authorization_url)
            }
        };
        Ok(AccountSignInStart {
            authorization_url,
            pending: serde_json::to_string(&pending)
                .map_err(|err| MailcalError::Config(err.to_string()))?,
        })
    }

    /// Completes a sign-in [`MailcalApp::begin_account_sign_in`] started: exchanges the redirect
    /// for a grant, puts it on `account_id` in place of its password or its old grant, connects
    /// with it and stores it, and runs the calendar and contacts rule again over it.
    ///
    /// **Blocking** (token exchange and a connect); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for a malformed `pending`, an account that is gone, or a
    /// server that says the sign-in was for another account; [`MailcalError::SignInRefused`] or
    /// [`MailcalError::Connect`] when the grant does not connect. The account is unchanged on
    /// every failure.
    pub fn complete_account_sign_in(
        &self,
        account_id: String,
        pending: String,
        callback_url: String,
    ) -> Result<(), MailcalError> {
        let pending: PendingSignIn =
            serde_json::from_str(&pending).map_err(|err| MailcalError::Config(err.to_string()))?;
        let replacement = match (pending, self.switchable(&account_id)?) {
            (PendingSignIn::Imap(pending), SignInConfig::Imap(config)) => {
                let signed_in = self.exchange_imap_login(pending, callback_url)?;
                named_as(signed_in.username.as_deref(), config.username())?;
                Replacement::Imap(config.with_grant(signed_in.grant))
            }
            (PendingSignIn::Jmap(pending), SignInConfig::Jmap(config)) => {
                let (fresh, named) = self.exchange_jmap_login(pending, callback_url)?;
                named_as(named.as_deref(), &config.email)?;
                Replacement::Jmap(JmapAccountConfig {
                    oauth: fresh.oauth,
                    password: None,
                    token: None,
                    ..*config
                })
            }
            _ => {
                return Err(MailcalError::Config(
                    "that sign-in was started for another account".to_owned(),
                ));
            }
        };
        self.install_credential(&account_id, replacement)
    }

    /// Switches `account_id` to signing in with `password`, in place of its grant: tests it
    /// against the account's servers, then stores it and drops the grant.
    ///
    /// **Blocking** (a connect); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an empty password or an account Settings cannot
    /// switch, [`MailcalError::SignInRefused`] when the server refuses it, and
    /// [`MailcalError::Connect`] when it cannot be reached. The account is unchanged on every
    /// failure.
    pub fn switch_account_to_password(
        &self,
        account_id: String,
        password: String,
    ) -> Result<(), MailcalError> {
        if password.is_empty() {
            return Err(MailcalError::Config("the password is empty".to_owned()));
        }
        let replacement = match self.switchable(&account_id)? {
            SignInConfig::Imap(config) => {
                Replacement::Imap(config.with_password_instead_of_grant(&password))
            }
            SignInConfig::Jmap(config) => Replacement::Jmap(JmapAccountConfig {
                password: Some(Secret::new(password)),
                token: None,
                oauth: None,
                ..*config
            }),
        };
        self.install_credential(&account_id, replacement)
    }

    /// Puts the credential of `config_toml`, a config a setup just produced for the mailbox
    /// `account_id` already is, on that account: what the "This account is already set up"
    /// refusal offers, so the sign-in or password just given replaces the existing account's
    /// rather than being thrown away.
    ///
    /// **Blocking** (a connect); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] when `config_toml` does not parse or is for another
    /// mailbox, and the errors of [`MailcalApp::switch_account_to_password`] otherwise.
    pub fn use_credential_from(
        &self,
        account_id: String,
        config_toml: String,
    ) -> Result<(), MailcalError> {
        let replacement = match self.switchable(&account_id)? {
            SignInConfig::Imap(config) => {
                let fresh = mailcal_account::load_str(&config_toml)
                    .map_err(|err| MailcalError::Config(err.to_string()))?;
                same_mailbox(fresh.derived_account_id(), config.derived_account_id())?;
                // A calendar or address book set up without mail carries its password on the DAV
                // endpoint alone.
                let password = fresh
                    .imap
                    .and_then(|imap| imap.password)
                    .or_else(|| fresh.caldav.and_then(|caldav| caldav.password))
                    .or_else(|| fresh.carddav.and_then(|carddav| carddav.password));
                match (fresh.oauth, password) {
                    (Some(grant), _) => Replacement::Imap(config.with_grant(grant)),
                    (None, Some(password)) => {
                        Replacement::Imap(config.with_password_instead_of_grant(password.expose()))
                    }
                    (None, None) => {
                        return Err(MailcalError::Config(
                            "that setup carries no credential".to_owned(),
                        ));
                    }
                }
            }
            SignInConfig::Jmap(config) => {
                let fresh = mailcal_account::load_jmap_str(&config_toml)
                    .map_err(|err| MailcalError::Config(err.to_string()))?;
                same_mailbox(fresh.derived_account_id(), config.derived_account_id())?;
                Replacement::Jmap(JmapAccountConfig {
                    oauth: fresh.oauth,
                    password: fresh.password,
                    token: fresh.token,
                    ..*config
                })
            }
        };
        self.install_credential(&account_id, replacement)
    }
}

impl MailcalApp {
    /// `account_id`'s config, when Settings can switch its sign-in.
    fn switchable(&self, account_id: &str) -> Result<SignInConfig, MailcalError> {
        self.registry.sign_in_config(account_id).ok_or_else(|| {
            MailcalError::Config("this account's sign-in cannot be switched here".to_owned())
        })
    }

    /// The IMAP half of [`MailcalApp::account_sign_in_options`].
    fn imap_sign_in_options(
        &self,
        account_id: &str,
        config: &AccountConfig,
    ) -> AccountSignInOptions {
        let Some(imap) = &config.imap else {
            return no_options();
        };
        let mut query = ImapAuthQuery {
            imap_host: imap.addr.clone(),
            imap_security: imap.security,
            email: imap.username.clone(),
            autoconfig_issuer: None,
        };
        if config.is_oauth() {
            let password = self
                .runtime
                .block_on(|| mailcal_account::imap_offers_password(&query));
            let provider_label = mailcal_oauth::provider_for_host(
                &mailcal_oauth::static_mail_providers(),
                &imap.server_name,
            )
            .map(|provider| provider.label.to_owned());
            return AccountSignInOptions {
                sign_in: true,
                provider_label,
                password,
            };
        }
        query.autoconfig_issuer =
            self.autoconfig_issuer(account_id, &imap.username, &imap.server_name);
        let decided = self
            .runtime
            .block_on(|| mailcal_account::decide_imap_auth(&query));
        let (sign_in, provider_label) = match decided {
            ImapAuth::SignIn { provider_label, .. } => (true, provider_label),
            ImapAuth::RegistrationNeeded { .. } | ImapAuth::Password => (false, None),
        };
        AccountSignInOptions {
            sign_in,
            provider_label,
            password: true,
        }
    }

    /// The issuer the provider's own autoconfig names for `email`, when detection finds the
    /// account's mail server there: what setup would have started a sign-in from. Looked for
    /// once per registration of `account_id`, by whichever of the options and the sign-in asks
    /// first.
    fn autoconfig_issuer(
        &self,
        account_id: &str,
        email: &str,
        server_name: &str,
    ) -> Option<String> {
        if let Some(issuer) = self.registry.sign_in_issuer(account_id) {
            return issuer;
        }
        let issuer = match self.detect_route(email, None) {
            SetupRecommendation::Imap {
                imap_host,
                oauth_issuer,
                ..
            } if crate::imap_oauth::host_only(&imap_host).eq_ignore_ascii_case(server_name) => {
                oauth_issuer
            }
            _ => None,
        };
        self.registry
            .remember_sign_in_issuer(account_id, issuer.clone());
        issuer
    }

    /// The sign-in request setup would have built for the password account `config`.
    fn imap_login_request(
        &self,
        account_id: &str,
        config: &AccountConfig,
    ) -> Result<ImapLoginRequest, MailcalError> {
        let imap = config.imap.as_ref().ok_or_else(|| {
            MailcalError::Config("this account signs in to no mailbox".to_owned())
        })?;
        Ok(ImapLoginRequest {
            email: imap.username.clone(),
            imap_host: imap.addr.clone(),
            smtp_host: config.smtp.as_ref().map(|smtp| smtp.addr.clone()),
            caldav_base_url: config.caldav.as_ref().map(|caldav| caldav.base_url.clone()),
            imap_security: Some(imap.security.into()),
            smtp_security: config.smtp.as_ref().map(|smtp| smtp.security.into()),
            oauth_issuer: self.autoconfig_issuer(account_id, &imap.username, &imap.server_name),
            carddav_base_url: config
                .carddav
                .as_ref()
                .map(|carddav| carddav.base_url.clone()),
            uses: Some(
                config
                    .capabilities()
                    .iter()
                    .map(crate::AccountCapability::from)
                    .collect(),
            ),
        })
    }

    /// Connects `account_id` with `replacement`, then stores it and installs it in place of the
    /// account's registry entry; on any failure the account keeps what it had.
    pub(crate) fn install_credential(
        &self,
        account_id: &str,
        replacement: Replacement,
    ) -> Result<(), MailcalError> {
        let (config_toml, grant, address) = match &replacement {
            Replacement::Imap(config) => (
                config.to_toml(),
                config.is_oauth(),
                config.username().to_owned(),
            ),
            Replacement::Jmap(config) => {
                (config.to_toml(), config.is_oauth(), config.email.clone())
            }
        };
        let config_toml = config_toml.map_err(|err| MailcalError::Config(err.to_string()))?;
        let sink = crate::token_sink::token_sink(&self.registry, &self.credential_store);
        let prepared =
            boot::prepare_stored_account(&config_toml, &sink, CredentialOrigin::FreshSignIn)?;
        // A grant can rotate during the connect, so it is stored from the registry, and it must
        // open this account's own mailbox; a password cannot rotate, so the bytes built here are
        // the ones to keep.
        if grant {
            self.install_signed_in_account(account_id, prepared, "sign-in", &address)?;
        } else {
            self.install_repaired_account(
                account_id,
                prepared,
                CredentialPersistence::Provided(config_toml),
                "sign-in",
            )?;
        }
        self.app.accounts_changed();
        log::info!(
            "sign-in: [{}] now signs in {}",
            mailcal_account::account_log_handle(account_id),
            if grant {
                "through its provider"
            } else {
                "with a password"
            },
        );
        Ok(())
    }
}

/// Refuses a sign-in whose server named another account than `address` in its token response
/// (`username`, draft-ietf-mailmaint-oauth-public §3.5). Nothing to check when it named none.
///
/// # Errors
///
/// Returns [`MailcalError::Config`] when the named account is another one.
pub(crate) fn named_as(named: Option<&str>, address: &str) -> Result<(), MailcalError> {
    match named {
        Some(named) if !addresses_match(named, address) => {
            log::warn!("sign-in: the server issued the grant for another account; discarding it");
            Err(MailcalError::Config(
                "that sign-in is for a different account than this one".to_owned(),
            ))
        }
        _ => Ok(()),
    }
}

/// Refuses a credential set up for another mailbox than the account it would replace.
fn same_mailbox<E: std::fmt::Display>(
    fresh: Result<engine_api::AccountId, E>,
    existing: Result<engine_api::AccountId, E>,
) -> Result<(), MailcalError> {
    let to_error = |err: E| MailcalError::Config(err.to_string());
    if fresh.map_err(to_error)? == existing.map_err(to_error)? {
        Ok(())
    } else {
        Err(MailcalError::Config(
            "that setup is for a different account than this one".to_owned(),
        ))
    }
}

const fn no_options() -> AccountSignInOptions {
    AccountSignInOptions {
        sign_in: false,
        provider_label: None,
        password: false,
    }
}

#[cfg(test)]
#[path = "account_sign_in_tests.rs"]
mod tests;
