//! `mailcal-oauth`: a small, provider-agnostic OAuth 2.0 **Authorization-Code with
//! PKCE** client for connecting hosted mail accounts.
//!
//! The engine is deliberately OAuth-agnostic; it takes a bearer access token and
//! nothing more (`north-star.md`): so **token acquisition and refresh are the host's
//! job**, and this crate is where the product core does that job. It owns the pure,
//! testable half of the flow:
//!
//! 1. [`OAuthClient::begin`] mints a [`Pkce`] pair + CSRF `state` and builds the authorization URL
//!    to open in the system browser.
//! 2. The **host** opens that URL (an OS auth session; `ASWebAuthenticationSession`, Chrome Custom
//!    Tabs, or a packaged-app protocol activation) and captures the redirect back to the registered
//!    custom scheme. This half is inherently platform-specific and lives in the native client, not
//!    here.
//! 3. [`OAuthClient::complete`] validates the returned `state`, then exchanges the authorization
//!    `code` (+ PKCE verifier) for a [`TokenSet`].
//! 4. [`OAuthClient::refresh`] re-mints an access token from a stored refresh token.
//!
//! Nothing here is Microsoft-specific beyond [`OAuthProviderConfig::microsoft`]; Gmail
//! and IMAP/SMTP `XOAUTH2` are just different endpoint/scope values.
//!
//! For a provider we have **no** pre-registered client with: a self-hosted JMAP server, or
//! any server we have never met; [`discover_protected_resource`] / [`discover_auth_server`] /
//! [`register_client`] find the endpoints and mint a client id from the standards
//! (RFC 9728 → RFC 8414 → RFC 7591) before step 1 above.
//!
//! The pre-registered clients we *do* have (Google's and Microsoft's) are injected at compile
//! time and live in [`credentials`], the only place in the tree that holds one. A build given
//! neither still connects mail: the routes that need a registration are simply not offered.

pub mod credentials;
mod discovery;
mod grant;
mod pkce;
mod provider;
mod reach;
mod register;
mod token;

use std::collections::HashMap;

pub use discovery::{
    AuthServerMetadata, DiscoveryError, ProtectedResource, discover_auth_server,
    discover_protected_resource, discovery_client,
};
pub use grant::{GrantRefusal, GrantedScopes};
pub use pkce::{Pkce, random_state};
pub use provider::{AuthStyle, GOOGLE_SCOPES, MICROSOFT_GRAPH_SCOPES, OAuthProviderConfig};
pub use reach::TokenRequestReach;
pub use register::{RegisteredClient, grants_mail_access, register_client, select_scopes};
use time::OffsetDateTime;
pub use token::{TokenSet, exchange_code, refresh};

/// A secret string (an access or refresh token) that redacts itself in `Debug`, so a
/// struct holding it can still derive/format `Debug` without leaking the token.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    /// Wraps a raw secret.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The underlying secret. Call only at the point of use (building a request or
    /// persisting to the OS keystore), never to log or display it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Debug for Secret {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// A failure somewhere in the OAuth flow.
#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    /// Building the shared TLS policy for OAuth HTTPS requests failed.
    #[error("oauth tls: {0}")]
    Tls(#[source] engine_tls::TlsError),
    /// The HTTP request to the token endpoint failed (a network/TLS error); retryable.
    #[error("oauth transport: {0}")]
    Transport(#[source] reqwest::Error),
    /// The server answered, but its response could not be read: the connection died
    /// after the status line and before the body was complete.
    ///
    /// Deliberately **not** [`OAuthError::Transport`], even though both wrap the same
    /// `reqwest::Error`: a response that started arriving proves the request *reached* the
    /// server and its handler ran. On a refresh that means the presented refresh token is
    /// spent and any rotated replacement was in the body we just lost. Retrying with the
    /// old token is then a replay, and against a ratcheting server (Fastmail) a replay
    /// revokes the grant. See [`OAuthError::reach`].
    #[error("oauth response lost after the server answered: {0}")]
    ResponseLost(#[source] reqwest::Error),
    /// The provider returned an OAuth error (RFC 6749 §5.2). `invalid_grant` means the
    /// grant is expired/revoked and the user must re-authenticate; others are usually
    /// configuration faults.
    #[error("oauth endpoint error: {error}; {}", .description.as_deref().unwrap_or("(no description)"))]
    Endpoint {
        /// The machine-readable error code (e.g. `invalid_grant`).
        error: String,
        /// The provider's human-readable detail, when present.
        description: Option<String>,
    },
    /// A success response could not be parsed as a token response: a protocol mismatch.
    #[error("decoding oauth response: {0}")]
    Decode(String),
    /// The redirect callback was malformed or forged: unparseable, missing `code`, or a
    /// `state` that doesn't match the one issued (a possible CSRF).
    #[error("oauth callback: {0}")]
    Callback(String),
}

/// The output of [`OAuthClient::begin`]: the URL to open, and the `state` + [`Pkce`]
/// the caller must hold until the redirect comes back (to validate `state` and to send
/// the verifier on the exchange).
#[derive(Debug)]
pub struct AuthRequest {
    /// The authorization URL to open in the system browser.
    pub authorization_url: String,
    /// The CSRF `state` issued for this request; the callback must echo it.
    pub state: String,
    /// The PKCE pair; its verifier is sent on [`OAuthClient::complete`].
    pub pkce: Pkce,
}

/// An OAuth client bound to one [`OAuthProviderConfig`], wrapping a reused HTTP client.
#[derive(Debug)]
pub struct OAuthClient {
    http: reqwest::Client,
    provider: OAuthProviderConfig,
}

impl OAuthClient {
    /// The scopes this client asks for on an authorization request.
    ///
    /// What a grant ends up carrying can be less (a service issues what it can) so this is the
    /// "requested" half [`GrantedScopes::from_response`] needs to read a response that named none.
    #[must_use]
    pub fn requested_scopes(&self) -> &[String] {
        &self.provider.scopes
    }

    /// Builds a client for `provider`.
    ///
    /// # Errors
    ///
    /// Returns [`OAuthError::Tls`] if the shared TLS policy cannot be built, or
    /// [`OAuthError::Transport`] if the underlying HTTP client cannot be built.
    pub fn new(provider: OAuthProviderConfig) -> Result<Self, OAuthError> {
        let tls = engine_tls::client_config(&engine_tls::TlsPolicy::bundled_and_system())
            .map_err(OAuthError::Tls)?;
        let http = tls
            .reqwest_builder()
            .build()
            .map_err(OAuthError::Transport)?;
        Ok(Self { http, provider })
    }

    /// This client's provider config.
    #[must_use]
    pub fn provider(&self) -> &OAuthProviderConfig {
        &self.provider
    }

    /// Starts a flow: mints a fresh PKCE pair + `state` and builds the authorization
    /// URL. The caller opens the URL and holds the returned `state`/verifier for
    /// [`OAuthClient::complete`]. `login_hint` (the address being connected, when known)
    /// pre-fills and targets that Microsoft account instead of showing the picker.
    #[must_use]
    pub fn begin(&self, login_hint: Option<&str>) -> AuthRequest {
        let pkce = Pkce::generate();
        let state = random_state();
        let authorization_url =
            self.provider
                .authorization_url(&state, pkce.challenge(), login_hint);
        AuthRequest {
            authorization_url,
            state,
            pkce,
        }
    }

    /// Completes a flow from the raw redirect `callback_url`: validates the echoed
    /// `state` against `expected_state`, then exchanges the `code` (+ PKCE verifier)
    /// for tokens. `now` timestamps the access token's absolute expiry.
    ///
    /// # Errors
    ///
    /// Returns [`OAuthError::Callback`] if the callback is malformed or its `state`
    /// doesn't match, [`OAuthError::Endpoint`] if the provider carried an `error` back
    /// or rejects the exchange, or [`OAuthError::Transport`]/[`OAuthError::Decode`] on a
    /// transport/parse failure.
    pub async fn complete(
        &self,
        callback_url: &str,
        expected_state: &str,
        pkce_verifier: &str,
        now: OffsetDateTime,
    ) -> Result<TokenSet, OAuthError> {
        let code = parse_callback(expected_state, callback_url)?;
        exchange_code(&self.http, &self.provider, &code, pkce_verifier, now).await
    }

    /// Redeems a stored `refresh_token` for a fresh [`TokenSet`].
    ///
    /// # Errors
    ///
    /// Returns [`OAuthError::Endpoint`] if the refresh token is revoked/expired
    /// (`invalid_grant`; re-authenticate), or a transport/decode error otherwise.
    pub async fn refresh(
        &self,
        refresh_token: &str,
        now: OffsetDateTime,
    ) -> Result<TokenSet, OAuthError> {
        refresh(&self.http, &self.provider, refresh_token, now).await
    }
}

/// Parses a redirect `callback_url`, returning the authorisation `code` once the
/// echoed `state` matches `expected_state`.
///
/// Handles the custom-scheme redirect the OS auth session hands back (e.g.
/// `eu.allodia.mailcal://oauth?code=…&state=…`). A provider `error` in the callback,
/// a missing `code`, a missing/mismatched `state`, or an unparseable URL are all
/// rejected.
///
/// # Errors
///
/// Returns [`OAuthError::Endpoint`] if the callback carries a provider `error`, or
/// [`OAuthError::Callback`] if it is unparseable, forged (`state` mismatch), or missing
/// the `code`.
pub fn parse_callback(expected_state: &str, callback_url: &str) -> Result<String, OAuthError> {
    let url = url::Url::parse(callback_url)
        .map_err(|err| OAuthError::Callback(format!("unparseable redirect URL: {err}")))?;
    let params: HashMap<String, String> = url.query_pairs().into_owned().collect();

    if let Some(error) = params.get("error") {
        return Err(OAuthError::Endpoint {
            error: error.clone(),
            description: params.get("error_description").cloned(),
        });
    }
    match params.get("state") {
        Some(state) if state == expected_state => {}
        Some(_) => {
            return Err(OAuthError::Callback(
                "state mismatch (possible CSRF)".to_owned(),
            ));
        }
        None => return Err(OAuthError::Callback("callback missing state".to_owned())),
    }
    params
        .get("code")
        .cloned()
        .ok_or_else(|| OAuthError::Callback("callback missing code".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_produces_a_url_that_embeds_the_generated_state_and_challenge() {
        let provider = OAuthProviderConfig::microsoft(
            "client-abc",
            "common",
            "eu.allodia.mailcal://oauth",
            MICROSOFT_GRAPH_SCOPES,
        );
        let client = OAuthClient::new(provider).unwrap();
        let req = client.begin(None);

        let parsed = url::Url::parse(&req.authorization_url).unwrap();
        let q: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(q["state"], req.state);
        assert_eq!(q["code_challenge"], req.pkce.challenge());
    }

    #[test]
    fn parse_callback_extracts_the_code_from_a_custom_scheme_redirect() {
        let code = parse_callback(
            "state-xyz",
            "eu.allodia.mailcal://oauth?code=the-code&state=state-xyz",
        )
        .unwrap();
        assert_eq!(code, "the-code");
    }

    #[test]
    fn parse_callback_handles_the_msauth_dotted_scheme_azure_generates() {
        // Azure auto-generates `msauth.<bundle-id>://auth` for a macOS/iOS platform (the
        // developer can't change it). The scheme contains dots; make sure the parser
        // still extracts code/state from it, not just the simpler `scheme://host` form.
        let code = parse_callback(
            "state-xyz",
            "msauth.eu.allodia.mailcal://auth?code=the-code&state=state-xyz",
        )
        .unwrap();
        assert_eq!(code, "the-code");
    }

    #[test]
    fn parse_callback_rejects_a_state_mismatch() {
        let err = parse_callback(
            "expected",
            "eu.allodia.mailcal://oauth?code=c&state=attacker",
        )
        .unwrap_err();
        assert!(matches!(err, OAuthError::Callback(_)));
    }

    #[test]
    fn parse_callback_rejects_a_missing_state() {
        let err = parse_callback("expected", "eu.allodia.mailcal://oauth?code=c").unwrap_err();
        assert!(matches!(err, OAuthError::Callback(msg) if msg.contains("state")));
    }

    #[test]
    fn parse_callback_surfaces_a_provider_error_in_the_redirect() {
        // The user cancelled / consent was denied → Microsoft redirects with `error`.
        let err = parse_callback(
            "state-xyz",
            "eu.allodia.mailcal://oauth?error=access_denied&error_description=user%20cancelled&state=state-xyz",
        )
        .unwrap_err();
        match err {
            OAuthError::Endpoint { error, description } => {
                assert_eq!(error, "access_denied");
                assert_eq!(description.as_deref(), Some("user cancelled"));
            }
            other => panic!("expected Endpoint, got {other:?}"),
        }
    }

    #[test]
    fn parse_callback_rejects_a_missing_code() {
        let err = parse_callback("s", "eu.allodia.mailcal://oauth?state=s").unwrap_err();
        assert!(matches!(err, OAuthError::Callback(msg) if msg.contains("code")));
    }

    #[test]
    fn invalid_grant_is_flagged_for_reauth() {
        let err = OAuthError::Endpoint {
            error: "invalid_grant".to_owned(),
            description: None,
        };
        assert!(err.is_invalid_grant());
        assert!(!OAuthError::Decode("x".to_owned()).is_invalid_grant());
    }
}
