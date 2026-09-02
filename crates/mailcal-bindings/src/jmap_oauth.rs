//! "Sign in with your provider" for a **JMAP** account, over the FFI.
//!
//! The shape mirrors [`crate::microsoft`]: the browser half of the flow belongs to the host,
//! because capturing a custom-scheme redirect is platform-specific, but with one step in
//! front of it that the Microsoft and Google flows do not need.
//!
//! Microsoft and Google are *integrated*: their endpoints, and this app's client id, are known
//! at build time. A JMAP server is not. It may be Fastmail, or a Stalwart someone runs on a
//! NAS, and we have no registration with either. So [`MailcalApp::begin_jmap_login`] first
//! **discovers** the whole thing from the standards; RFC 9728 to find the authorization
//! server, RFC 8414 to read its endpoints, RFC 7591 to register this install as a client, and
//! only then builds the PKCE authorization URL. See `mailcal_oauth::discovery`.
//!
//! # Discoverable, not guaranteed, and why that shapes the API
//!
//! Every step of that chain is optional for a server to support. A server may publish no
//! metadata, may not offer open registration, or may withdraw it tomorrow. So discovery
//! failure is **not** an error the user should have to interpret: it means "this server
//! doesn't do this", and the setup form's password/API-token field, which still works
//! everywhere; stays right there. [`MailcalApp::jmap_oauth_available`] answers exactly that
//! question so a client can decide whether to *show* the sign-in button at all.
//!
//! # What the host does with the result
//!
//! [`MailcalApp::complete_jmap_login`] returns the same `[jmap]` config TOML the manual form
//! produces, so a client adds and stores it through the code path it already has
//! (`add_account` + its existing JMAP secure-store write). A **rotated** refresh token is
//! re-persisted through the host's [`AccountCredentialStore`](crate::AccountCredentialStore),
//! which every core takes at construction; without one an account whose server rotates would
//! die at the next launch.
//!
//! # Signing an existing account back in
//!
//! A grant that has expired or been revoked is not a setup problem, so it does not go back
//! through the setup form: [`reauth`] re-runs the *same* authorisation against the account's
//! **persisted** grant and swaps the result in place. See that module.

use mailcal_account::{JmapAccountConfig, OAuthGrant, Secret};
use mailcal_oauth::OAuthClient;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{MailcalApp, MailcalError};

mod discovery;
mod reauth;

use discovery::{Discovered, discover_and_register};

/// The client name shown on the provider's consent screen. The product name, per the brand
/// rule: this is user-facing copy on someone else's page.
const CLIENT_NAME: &str = "Allodia Mail & Calendar";

/// The JMAP session resource path (RFC 8620 §2), which is also the protected resource whose
/// `401` names the authorization server.
const SESSION_PATH: &str = "/.well-known/jmap";

/// What [`MailcalApp::begin_jmap_login`] returns: the URL to open in the platform auth
/// session, and an opaque handle to pass back to `complete_jmap_login`.
#[derive(uniffi::Record)]
pub struct JmapLoginStart {
    /// The authorization URL to open in the platform auth session.
    pub authorization_url: String,
    /// An opaque handle (discovered endpoints + registered client id + PKCE verifier).
    /// **Transient; hold it in memory only.** It carries the PKCE verifier.
    pub pending: String,
}

/// The transient state carried between begin and complete. Round-tripped through the host as
/// the opaque `pending` handle, and never persisted; it holds the PKCE verifier.
#[derive(Serialize, Deserialize)]
struct PendingJmapLogin {
    email: String,
    base_url: String,
    client_id: String,
    client_secret: Option<String>,
    authorize_endpoint: String,
    token_endpoint: String,
    redirect_uri: String,
    scopes: Vec<String>,
    /// The RFC 8707 resource indicator discovered for this server, carried across the browser
    /// hop so the exchange can name the same target the authorization request did.
    resource: Option<String>,
    /// The issuer the redirect's `iss` must name (RFC 9207), when the server advertised that it
    /// sends one. Carried across the hop because the check happens on the way back.
    issuer: Option<String>,
    state: String,
    verifier: String,
}

impl PendingJmapLogin {
    /// Rebuilds the grant this pending login will become, given the tokens it was exchanged
    /// for. Kept next to the struct so the fields carried across the browser hop and the
    /// fields persisted afterwards cannot drift apart.
    fn into_grant(self, refresh_token: String) -> (String, String, OAuthGrant) {
        let grant = OAuthGrant {
            client_id: self.client_id,
            client_secret: self.client_secret.map(Secret::new),
            refresh_token: Secret::new(refresh_token),
            authorize_endpoint: self.authorize_endpoint,
            token_endpoint: self.token_endpoint,
            redirect_uri: self.redirect_uri,
            scopes: self.scopes,
            resource: self.resource,
            issuer: self.issuer,
        };
        (self.email, self.base_url, grant)
    }
}

/// Builds the PKCE authorization request for an already-assembled `grant`, and packs everything
/// the completion will need into the opaque `pending` handle.
///
/// The two entry points that need this: a first sign-in (which has just *discovered* the grant)
/// and a re-authentication (which *loaded* it) differ only in where the grant came from, so the
/// authorisation half is written once. That is not only tidiness: the fields carried across the
/// browser hop must match the ones the grant is rebuilt from, and duplicating this is exactly how
/// a `redirect_uri` or a `resource` silently stops being replayed.
fn start_login(
    email: String,
    base_url: String,
    grant: &OAuthGrant,
) -> Result<JmapLoginStart, MailcalError> {
    let oauth = OAuthClient::new(grant.provider_config())
        .map_err(|err| MailcalError::Config(err.to_string()))?;
    // The address is known, so target it rather than making the user pick.
    let request = oauth.begin(Some(&email));
    let pending = PendingJmapLogin {
        email,
        base_url,
        client_id: grant.client_id.clone(),
        client_secret: grant
            .client_secret
            .as_ref()
            .map(|secret| secret.expose().to_owned()),
        authorize_endpoint: grant.authorize_endpoint.clone(),
        token_endpoint: grant.token_endpoint.clone(),
        redirect_uri: grant.redirect_uri.clone(),
        scopes: grant.scopes.clone(),
        resource: grant.resource.clone(),
        issuer: grant.issuer.clone(),
        state: request.state,
        verifier: request.pkce.verifier().to_owned(),
    };
    Ok(JmapLoginStart {
        authorization_url: request.authorization_url,
        pending: serde_json::to_string(&pending)
            .map_err(|err| MailcalError::Config(err.to_string()))?,
    })
}

#[uniffi::export]
impl MailcalApp {
    /// Whether the JMAP server for `email`/`server_url` offers discoverable OAuth sign-in.
    ///
    /// A cheap pre-flight so a client can decide whether to *show* a "Sign in with your
    /// provider" button rather than offering one that dead-ends. Runs the discovery chain
    /// short of registration. **Blocking**; call it off the main thread, exactly like
    /// `detect_account_settings`. Never throws: any failure is `false`.
    pub fn jmap_oauth_available(&self, email: String, server_url: Option<String>) -> bool {
        let Ok(base_url) = mailcal_account::jmap_base_url(&email, server_url.as_deref()) else {
            return false;
        };
        self.runtime.block_on(async {
            let Ok(http) = mailcal_oauth::discovery_client() else {
                return false;
            };
            let session_url = format!("{}{SESSION_PATH}", base_url.trim_end_matches('/'));
            let protected =
                match mailcal_oauth::discover_protected_resource(&http, &session_url).await {
                    Ok(protected) => protected,
                    Err(err) => {
                        log::debug!("jmap oauth: no sign-in offered for {session_url}; {err}");
                        return false;
                    }
                };
            match mailcal_oauth::discover_auth_server(&http, &protected.issuer).await {
                // Registration is what actually mints our client id, so a server that
                // advertises no registration endpoint cannot be offered sign-in.
                Ok(metadata) => {
                    let offered = metadata.registration_endpoint.is_some();
                    log::info!(
                        "jmap oauth: {} advertises sign-in: {offered}",
                        protected.issuer
                    );
                    offered
                }
                Err(err) => {
                    log::debug!(
                        "jmap oauth: no sign-in offered by {}; {err}",
                        protected.issuer
                    );
                    false
                }
            }
        })
    }

    /// Starts a JMAP OAuth sign-in: discovers the server's authorization server, registers
    /// this install as a client, and builds the PKCE authorization URL to open.
    ///
    /// `server_url` may be `None`/empty, in which case it is derived from the email domain;
    /// the same rule the manual form uses. `redirect_uri` is the platform's registered custom
    /// scheme, whose redirect the host's auth session captures.
    ///
    /// **Blocking** (it makes several network round trips); call it off the main thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Connect`] if the server does not support discoverable sign-in
    /// at any step. That is an expected outcome, not a defect: the caller falls back to the
    /// password/API-token field.
    pub fn begin_jmap_login(
        &self,
        email: String,
        server_url: Option<String>,
        redirect_uri: String,
    ) -> Result<JmapLoginStart, MailcalError> {
        let base_url = mailcal_account::jmap_base_url(&email, server_url.as_deref())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        log::info!("jmap oauth: sign-in requested for the server at {base_url}");
        let Discovered {
            metadata,
            scopes,
            client,
            resource,
        } = self
            .runtime
            .block_on(discover_and_register(&base_url, &redirect_uri))?;

        let grant = OAuthGrant {
            client_id: client.client_id,
            client_secret: client.client_secret.map(Secret::new),
            // Not yet issued: a placeholder purely so the provider config can be built from
            // one place. It is never read before `complete_jmap_login` replaces it.
            refresh_token: Secret::new(String::new()),
            authorize_endpoint: metadata.authorization_endpoint,
            token_endpoint: metadata.token_endpoint,
            redirect_uri,
            scopes,
            resource,
            // Only when the server said it sends one: absent then means "nothing to compare",
            // while a `None` here on a server that does send one would skip the check.
            issuer: metadata
                .issuer_parameter_supported
                .then(|| metadata.issuer.clone()),
        };
        log::info!("jmap oauth: opening the authorization page; awaiting the redirect");
        start_login(email, base_url, &grant)
    }

    /// Completes a JMAP OAuth sign-in: validates the redirect, exchanges the code for tokens,
    /// and returns the `[jmap]` config TOML.
    ///
    /// The TOML is byte-for-byte the shape the manual form produces (plus a `[jmap.oauth]`
    /// section), so the host adds it with `add_account` and stores it with the same secure
    /// store write it already uses: no new storage path.
    ///
    /// **Blocking**; call it off the main thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `pending` is malformed, or
    /// [`MailcalError::Connect`] if the user cancelled, the exchange failed, or the server
    /// issued no refresh token (without one the account would break within the hour).
    pub fn complete_jmap_login(
        &self,
        pending: String,
        callback_url: String,
    ) -> Result<String, MailcalError> {
        self.exchange_jmap_login(pending, callback_url)?
            .to_toml()
            .map_err(|err| MailcalError::Config(err.to_string()))
    }
}

impl MailcalApp {
    /// The exchange half of a completed JMAP sign-in, shared by [`Self::complete_jmap_login`]
    /// (which hands the config back as TOML for the host to add) and the re-authentication path
    /// (which swaps it into a live account itself). Validates the redirect, exchanges the code,
    /// and assembles the account config the grant belongs to.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `pending` is malformed, or [`MailcalError::Connect`]
    /// if the user cancelled, the exchange failed, or the server issued no refresh token.
    fn exchange_jmap_login(
        &self,
        pending: String,
        callback_url: String,
    ) -> Result<JmapAccountConfig, MailcalError> {
        let pending: PendingJmapLogin =
            serde_json::from_str(&pending).map_err(|err| MailcalError::Config(err.to_string()))?;
        let grant_shell = OAuthGrant {
            client_id: pending.client_id.clone(),
            client_secret: pending.client_secret.clone().map(Secret::new),
            refresh_token: Secret::new(String::new()),
            authorize_endpoint: pending.authorize_endpoint.clone(),
            token_endpoint: pending.token_endpoint.clone(),
            redirect_uri: pending.redirect_uri.clone(),
            scopes: pending.scopes.clone(),
            resource: pending.resource.clone(),
            issuer: pending.issuer.clone(),
        };
        let oauth = OAuthClient::new(grant_shell.provider_config())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        log::info!(
            "jmap oauth: redirect received; exchanging the code at {} (resource indicator: {})",
            pending.token_endpoint,
            pending.resource.as_deref().unwrap_or("(none)"),
        );
        let tokens = self
            .runtime
            .block_on(oauth.complete(
                &callback_url,
                &pending.state,
                &pending.verifier,
                OffsetDateTime::now_utc(),
            ))
            .map_err(|err| {
                // The single most valuable line in the whole flow for support: the server's own
                // machine-readable reason. `invalid_target` means the RFC 8707 resource indicator
                // was missing or wrong; `invalid_grant` means the code was stale or replayed.
                // Without this the user only ever sees "signing in didn't work".
                log::warn!("jmap oauth: token exchange FAILED; {err}");
                MailcalError::Connect(err.to_string())
            })?;
        // `offline_access` is always requested, so a refresh token is mandatory; without one
        // the account would stop syncing about an hour after setup, which is far worse to
        // debug than failing here.
        let refresh_token = tokens.refresh_token.ok_or_else(|| {
            log::warn!(
                "jmap oauth: exchange succeeded but the server issued NO refresh token; granted                  scope(s): [{}]",
                tokens.scope,
            );
            MailcalError::Connect(
                "the server issued no refresh token (offline_access was requested)".to_owned(),
            )
        })?;
        log::info!(
            "jmap oauth: sign-in complete; access token valid until {}, granted scope(s): [{}]",
            tokens.expires_at,
            tokens.scope,
        );
        let (email, base_url, grant) = pending.into_grant(refresh_token.expose().to_owned());
        Ok(JmapAccountConfig {
            email,
            base_url,
            password: None,
            token: None,
            oauth: Some(grant),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending() -> PendingJmapLogin {
        PendingJmapLogin {
            email: "alice@example.com".to_owned(),
            base_url: "https://api.example.com".to_owned(),
            client_id: "client-abc".to_owned(),
            client_secret: None,
            authorize_endpoint: "https://api.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://api.example.com/oauth/refresh".to_owned(),
            redirect_uri: "eu.allodia.mailcal://jmap-oauth".to_owned(),
            scopes: vec!["offline_access".to_owned()],
            resource: Some("https://api.example.com/jmap/session".to_owned()),
            issuer: None,
            state: "state-xyz".to_owned(),
            verifier: "verifier".to_owned(),
        }
    }

    #[test]
    fn the_pending_handle_round_trips_through_the_host() {
        // The host carries this opaque across a browser hop, so it must survive
        // serialization exactly: a dropped verifier or state would fail the exchange with a
        // message that points nowhere near the cause.
        let encoded = serde_json::to_string(&pending()).unwrap();
        let decoded: PendingJmapLogin = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.state, "state-xyz");
        assert_eq!(decoded.verifier, "verifier");
        assert_eq!(decoded.redirect_uri, "eu.allodia.mailcal://jmap-oauth");
    }

    #[test]
    fn the_completed_grant_persists_the_discovered_endpoints_and_client_id() {
        // The whole point of persisting these: a later launch refreshes without repeating
        // discovery, and never re-registers a client (the brief's "persist the DCR client id
        // per install").
        let (email, base_url, grant) = pending().into_grant("rt-value".to_owned());
        assert_eq!(email, "alice@example.com");
        assert_eq!(base_url, "https://api.example.com");
        assert_eq!(grant.client_id, "client-abc");
        assert_eq!(grant.refresh_token.expose(), "rt-value");
        assert_eq!(
            grant.token_endpoint,
            "https://api.example.com/oauth/refresh"
        );
        // The RFC 8707 target must be persisted, not merely used once: every refresh re-sends
        // it, and a grant that loses it starts failing `invalid_target` an hour after setup.
        assert_eq!(
            grant.resource.as_deref(),
            Some("https://api.example.com/jmap/session")
        );
    }

    #[test]
    fn the_config_toml_carries_the_grant_and_no_stored_secret() {
        let (email, base_url, grant) = pending().into_grant("rt-value".to_owned());
        let toml = mailcal_account::JmapAccountConfig {
            email,
            base_url,
            password: None,
            token: None,
            oauth: Some(grant),
        }
        .to_toml()
        .unwrap();
        let parsed = mailcal_account::load_jmap_str(&toml).unwrap();

        // An OAuth account stores no long-lived password/token: only the grant.
        assert!(parsed.password.is_none());
        assert!(parsed.token.is_none());
        assert!(parsed.is_oauth());
        let oauth = parsed.oauth.unwrap();
        assert_eq!(oauth.client_id, "client-abc");
        assert_eq!(oauth.refresh_token.expose(), "rt-value");
        assert_eq!(oauth.scopes, vec!["offline_access".to_owned()]);
        // The redirect URI must survive verbatim: a refresh replays it, and a server that
        // sees a different one rejects the grant.
        assert_eq!(oauth.redirect_uri, "eu.allodia.mailcal://jmap-oauth");
    }

    /// The re-authentication path hands `start_login` a **persisted** grant and no network call
    /// has happened. What must survive into the authorization request, and into the `pending`
    /// the exchange is rebuilt from, is that grant's own client id, endpoint, redirect and
    /// RFC 8707 resource: re-discovering or re-registering would mint a second client id on the
    /// user's account, and a dropped resource fails the exchange with `invalid_target`.
    #[test]
    fn an_authorization_is_built_from_the_grant_it_is_given() {
        let (email, base_url, grant) = pending().into_grant("rt-value".to_owned());

        let start = start_login(email, base_url, &grant).unwrap();

        assert!(
            start
                .authorization_url
                .starts_with("https://api.example.com/oauth/authorize?"),
            "the stored authorize endpoint is where the user is sent: {}",
            start.authorization_url,
        );
        assert!(start.authorization_url.contains("client_id=client-abc"));
        let decoded: PendingJmapLogin = serde_json::from_str(&start.pending).unwrap();
        assert_eq!(decoded.client_id, "client-abc");
        assert_eq!(decoded.redirect_uri, "eu.allodia.mailcal://jmap-oauth");
        assert_eq!(
            decoded.resource.as_deref(),
            Some("https://api.example.com/jmap/session"),
        );
        assert_eq!(decoded.scopes, vec!["offline_access".to_owned()]);
    }

    #[test]
    fn the_toml_never_shows_a_secret_in_a_debug_dump() {
        let (email, base_url, grant) = pending().into_grant("rt-value".to_owned());
        let config = mailcal_account::JmapAccountConfig {
            email,
            base_url,
            password: None,
            token: None,
            oauth: Some(grant),
        };
        assert!(!format!("{config:?}").contains("rt-value"));
    }
}
