//! Signing an existing Microsoft or Google account in again, for what it is used for and anything
//! it is to be used for as well.
//!
//! `complete_microsoft_login` and `complete_google_login` add an account: they reset its sync
//! depth and start a visible first download, which is right for an account that did not exist a
//! moment ago and wrong for one that did. This is the other door: it swaps the grant in place
//! through `install_repaired_account`, the path a JMAP re-sign-in and a replaced
//! password already take, so the account keeps its id, settings, downloaded mail and links, and
//! catches up rather than downloading again.

use mailcal_account::{Capabilities, Capability};
use serde::{Deserialize, Serialize};

use crate::{
    AccountCapability, MailcalApp, MailcalError, account_repair::CredentialPersistence, boot,
    google, microsoft,
};

/// What [`MailcalApp::begin_account_consent`] returns: the authorization URL to open, and an
/// opaque `pending` handle to pass to [`MailcalApp::complete_account_consent`].
#[derive(uniffi::Record)]
pub struct AccountConsentStart {
    /// The authorization URL to open, exactly as for a first sign-in at the same provider.
    pub authorization_url: String,
    /// An opaque handle naming the account and carrying the sign-in's PKCE verifier. Transient;
    /// hold it in memory only.
    pub pending: String,
}

/// Which provider an account signs in at, as the `pending` handle names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Provider {
    Microsoft,
    Google,
}

impl Provider {
    const fn label(self) -> &'static str {
        match self {
            Self::Microsoft => "microsoft",
            Self::Google => "google",
        }
    }
}

/// What a sign-in for an existing account needs to know about it, cloned out of the registry.
#[derive(Debug, Clone)]
pub(crate) struct OAuthAccount {
    provider: Provider,
    email: String,
    /// Microsoft's tenant; empty for Google.
    tenant: String,
    capabilities: Capabilities,
    shape: mailcal_account::AccountShape,
}

impl OAuthAccount {
    /// The account `entry` describes, when it signs in at Microsoft or Google.
    pub(crate) fn of(entry: &crate::ConnectedAccount) -> Option<Self> {
        match entry {
            crate::ConnectedAccount::Microsoft { config, .. } => Some(Self {
                provider: Provider::Microsoft,
                email: config.email.clone(),
                tenant: config.tenant.clone(),
                capabilities: config.capabilities(),
                shape: config.shape.clone(),
            }),
            crate::ConnectedAccount::Google { config, .. } => Some(Self {
                provider: Provider::Google,
                email: config.email.clone(),
                tenant: String::new(),
                capabilities: config.capabilities(),
                shape: config.shape.clone(),
            }),
            crate::ConnectedAccount::Imap { .. } | crate::ConnectedAccount::Jmap { .. } => None,
        }
    }
}

/// The handle round-tripped between begin and complete.
#[derive(Serialize, Deserialize)]
struct PendingConsent {
    account_id: String,
    provider: Provider,
    /// The provider's own `pending` handle.
    login: String,
}

#[uniffi::export]
impl MailcalApp {
    /// Starts signing the existing Microsoft or Google account `account_id` in again, asking for
    /// what it is used for plus `adding`. `redirect_uri` is the host's, as for
    /// [`begin_microsoft_login`](crate::begin_microsoft_login) and
    /// [`begin_google_login`](crate::begin_google_login): the custom scheme for Microsoft, the
    /// loopback port the host bound for a Google Desktop client. The account's address is sent as
    /// the `login_hint`.
    ///
    /// `adding` empty signs the account in again for what it already does: the remedy for an
    /// expired sign-in or a withheld permission.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `account_id` is unknown or is not a Microsoft or Google
    /// account, or if this build carries no registration for its provider.
    pub fn begin_account_consent(
        &self,
        account_id: String,
        redirect_uri: String,
        adding: Vec<AccountCapability>,
    ) -> Result<AccountConsentStart, MailcalError> {
        let account = self.registry.oauth_account(&account_id).ok_or_else(|| {
            MailcalError::Config("no Microsoft or Google account by that id".to_owned())
        })?;
        let chosen: Capabilities = account
            .capabilities
            .iter()
            .chain(adding.into_iter().map(Capability::from))
            .collect();
        log::info!(
            "consent: [{}] signing a {} account in again for {} use(s)",
            mailcal_account::account_log_handle(&account_id),
            account.provider.label(),
            chosen.iter().count(),
        );
        let (authorization_url, login) = match account.provider {
            Provider::Microsoft => {
                let start = microsoft::start(
                    Some(account.tenant),
                    redirect_uri,
                    Some(account.email),
                    Some(chosen),
                )?;
                (start.authorization_url, start.pending)
            }
            Provider::Google => {
                let start = google::start(redirect_uri, Some(account.email), Some(chosen))?;
                (start.authorization_url, start.pending)
            }
        };
        let pending = serde_json::to_string(&PendingConsent {
            account_id,
            provider: account.provider,
            login,
        })
        .map_err(|err| MailcalError::Config(err.to_string()))?;
        Ok(AccountConsentStart {
            authorization_url,
            pending,
        })
    }

    /// Completes a sign-in started by [`MailcalApp::begin_account_consent`]: exchanges the
    /// redirect, refuses a sign-in as a different address, connects the account with the new
    /// grant, stores it, and retracts the prompts the grant answers.
    ///
    /// **Blocking** (token exchange plus a connect); call it off the main thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `pending` is malformed, the account has been removed
    /// meanwhile, or the sign-in was for a different address; [`MailcalError::Connect`] if the
    /// exchange, the connect or the credential write failed. The account's stored grant is left
    /// as it was on every failure.
    pub fn complete_account_consent(
        &self,
        pending: String,
        callback_url: String,
    ) -> Result<(), MailcalError> {
        let pending: PendingConsent =
            serde_json::from_str(&pending).map_err(|err| MailcalError::Config(err.to_string()))?;
        // The person may have removed the account while the browser was up.
        let stored = self
            .registry
            .oauth_account(&pending.account_id)
            .filter(|account| account.provider == pending.provider)
            .ok_or_else(|| MailcalError::Config("that account is no longer set up".to_owned()))?;
        let now = time::OffsetDateTime::now_utc();
        let signed_in = match pending.provider {
            Provider::Microsoft => self
                .runtime
                .block_on(microsoft::authorize(&pending.login, &callback_url, now))
                .and_then(|authorized| {
                    let mut config = authorized.config;
                    same_address(&stored.email, &config.email)?;
                    crate::consent::keep_stored_shape(
                        &mut config.shape,
                        Some(stored.shape.clone()),
                    );
                    config
                        .to_toml()
                        .map_err(|err| MailcalError::Config(err.to_string()))
                }),
            Provider::Google => self
                .runtime
                .block_on(google::authorize(&pending.login, &callback_url, now))
                .and_then(|authorized| {
                    let mut config = authorized.config;
                    same_address(&stored.email, &config.email)?;
                    crate::consent::keep_stored_shape(
                        &mut config.shape,
                        Some(stored.shape.clone()),
                    );
                    config
                        .to_toml()
                        .map_err(|err| MailcalError::Config(err.to_string()))
                }),
        };
        let config_toml = signed_in.inspect_err(|err| {
            log::warn!(
                "consent: [{}] the sign-in did not complete ({err}); the stored grant is unchanged",
                mailcal_account::account_log_handle(&pending.account_id),
            );
        })?;
        let sink = crate::token_sink::token_sink(&self.registry, &self.credential_store);
        let prepared = boot::prepare_stored_account(
            &config_toml,
            &sink,
            mailcal_account::CredentialOrigin::FreshSignIn,
        )?;
        let id = prepared.account.id.clone();
        self.install_repaired_account(
            &pending.account_id,
            prepared,
            CredentialPersistence::RegisteredGrant,
            pending.provider.label(),
        )?;
        // The new grant asks for mail's write and send scopes whenever the account is used for
        // mail; a send it still cannot make raises the prompt again.
        self.app.clear_mail_reauth_required(&id);
        Ok(())
    }
}

/// Refuses a sign-in completed as a different address than the account's own: `login_hint`
/// targets an address, it does not pin one, and filing another mailbox's grant under this
/// account would show one address while reading another person's mail.
fn same_address(stored: &str, signed_in: &str) -> Result<(), MailcalError> {
    if stored.trim().eq_ignore_ascii_case(signed_in.trim()) {
        Ok(())
    } else {
        log::warn!("consent: the sign-in was for a different account; discarding the new grant");
        Err(MailcalError::Config(
            "that sign-in is for a different account than the one being signed in again".to_owned(),
        ))
    }
}

#[cfg(test)]
#[path = "app_accounts_consent_tests.rs"]
mod tests;
