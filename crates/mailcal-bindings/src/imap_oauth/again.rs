//! The halves of an IMAP sign-in an existing account reuses: the code exchange, and an
//! authorization request built from the account's own stored grant.

use mailcal_account::{AccountConfig, AccountSetup, ImapAuthQuery, OAuthGrant};
use mailcal_oauth::OAuthClient;
use time::OffsetDateTime;

use super::{
    ImapLoginStart, PendingImapLogin,
    discovery::{Endpoints, resources_again},
    grant_shell, start_login,
};
use crate::{MailcalApp, MailcalError};

/// A completed IMAP sign-in: the account it describes, its grant, and the account the server
/// named in its token response, when it named one.
pub(crate) struct ImapSignedIn {
    pub(crate) setup: AccountSetup,
    pub(crate) grant: OAuthGrant,
    pub(crate) username: Option<String>,
}

impl MailcalApp {
    /// Validates the redirect a sign-in came back with and exchanges its code for a grant.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `pending` is malformed, or [`MailcalError::Connect`]
    /// if the user cancelled, the exchange failed, or the server issued no refresh token.
    pub(crate) fn exchange_imap_login(
        &self,
        pending: String,
        callback_url: String,
    ) -> Result<ImapSignedIn, MailcalError> {
        let pending: PendingImapLogin =
            serde_json::from_str(&pending).map_err(|err| MailcalError::Config(err.to_string()))?;
        let grant_shell = grant_shell(&pending);
        let oauth = OAuthClient::new(grant_shell.provider_config())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        log::info!(
            "imap oauth: redirect received; exchanging the code at {}",
            pending.token_endpoint,
        );
        let tokens = self
            .runtime
            .block_on(|| {
                oauth.complete(
                    &callback_url,
                    &pending.state,
                    &pending.verifier,
                    OffsetDateTime::now_utc(),
                )
            })
            .map_err(|err| {
                // The server's own machine-readable reason, which is the single most useful
                // line in the whole flow for support: `invalid_grant` means the code was
                // stale or replayed, and a callback rejection names which check failed.
                log::warn!("imap oauth: token exchange FAILED; {err}");
                MailcalError::Connect(err.to_string())
            })?;
        // `offline_access` is always requested, so a refresh token is mandatory: without one
        // the account stops syncing about an hour after setup, which is far worse to diagnose
        // than failing here.
        let refresh_token = tokens.refresh_token.ok_or_else(|| {
            log::warn!(
                "imap oauth: exchange succeeded but the server issued NO refresh token; granted scope(s): [{}]",
                tokens.scope,
            );
            MailcalError::Connect(
                "the server issued no refresh token (offline_access was requested)".to_owned(),
            )
        })?;
        log::info!(
            "imap oauth: sign-in complete; access token valid until {}, granted scope(s): [{}]",
            tokens.expires_at,
            tokens.scope,
        );
        let username = tokens.username.clone();
        let (setup, grant) = pending.into_grant(refresh_token.expose().to_owned());
        Ok(ImapSignedIn {
            setup,
            grant,
            username,
        })
    }
}

impl MailcalApp {
    /// The authorization request that signs `config`'s account in again, built from its stored
    /// `grant`: the same issuer, registered client, redirect and scopes, with no second
    /// registration, as a JMAP account's is (`docs/jmap.md` rule 8). The endpoints it names
    /// follow setup's rule over the issuer's metadata, read again.
    ///
    /// **Blocking** (one metadata read, and the DAV servers' own when they are named).
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an account used for no mail, which has no mailbox to
    /// sign in to, or a grant the authorization request cannot be built from.
    pub(crate) fn sign_in_again(
        &self,
        config: &AccountConfig,
        grant: &OAuthGrant,
    ) -> Result<ImapLoginStart, MailcalError> {
        let imap = config.imap.as_ref().ok_or_else(|| {
            MailcalError::Config("this account signs in to no mailbox".to_owned())
        })?;
        let query = ImapAuthQuery {
            imap_host: imap.addr.clone(),
            imap_security: imap.security,
            email: imap.username.clone(),
            autoconfig_issuer: None,
        };
        let caldav = config.caldav.as_ref().map(|caldav| caldav.base_url.clone());
        let carddav = config
            .carddav
            .as_ref()
            .map(|carddav| carddav.base_url.clone());
        let smtp_host = config.smtp.as_ref().map(|smtp| smtp.addr.clone());
        let endpoints = Endpoints {
            smtp_host: smtp_host.as_deref(),
            caldav: caldav.as_deref(),
            carddav: carddav.as_deref(),
        };
        let resources = self.runtime.block_on(|| {
            resources_again(
                grant.authorization_server.as_deref(),
                &query,
                &endpoints,
                &grant.scopes,
            )
        });
        let starttls = |security| matches!(security, mailcal_account::ConnectionSecurity::StartTls);
        log::info!("imap oauth: signing an existing account in again at its stored issuer");
        start_login(PendingImapLogin {
            email: imap.username.clone(),
            imap_host: imap.addr.clone(),
            smtp_host,
            caldav_base_url: caldav,
            carddav_base_url: carddav,
            uses: Some(crate::account_capability::names(&config.capabilities())),
            imap_starttls: starttls(imap.security),
            smtp_starttls: config
                .smtp
                .as_ref()
                .is_some_and(|smtp| starttls(smtp.security)),
            client_id: grant.client_id.clone(),
            client_secret: grant
                .client_secret
                .as_ref()
                .map(|secret| secret.expose().to_owned()),
            authorize_endpoint: grant.authorize_endpoint.clone(),
            token_endpoint: grant.token_endpoint.clone(),
            redirect_uri: grant.redirect_uri.clone(),
            scopes: grant.scopes.clone(),
            issuer: grant.issuer.clone(),
            authorization_server: grant.authorization_server.clone(),
            protected_resources: grant.protected_resources.clone(),
            resources,
            state: String::new(),
            verifier: String::new(),
        })
    }
}
