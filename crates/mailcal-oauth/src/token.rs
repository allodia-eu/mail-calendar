//! The back-channel token exchanges: authorisation `code` → tokens, and
//! `refresh_token` → fresh tokens.
//!
//! Both are `application/x-www-form-urlencoded` POSTs to the provider's token
//! endpoint. Every client is a **public client** protected by PKCE, so no *confidential* secret
//! is involved, but a Google _Desktop_ client (the macOS/Windows loopback flow) carries a
//! **non-confidential** `client_secret` that Google's token endpoint requires anyway (see
//! [`OAuthProviderConfig::client_secret`]); it rides on both the code exchange **and** the
//! refresh when present, so the account doesn't break an hour after setup. A non-2xx body is
//! parsed for the standard OAuth `error`/`error_description` so a caller can tell an expired
//! grant (`invalid_grant` → re-auth) from a transient transport failure (→ retry).

use serde::Deserialize;
use time::{Duration, OffsetDateTime};

use crate::{OAuthError, Secret, provider::OAuthProviderConfig};

/// The tokens issued for one account, with the access token's absolute expiry
/// resolved from the response's `expires_in` so a caller can decide when to refresh.
///
/// The `refresh_token` is `Option` because a refresh response may omit it (the
/// provider keeps the previous one valid); callers carry the prior refresh token
/// forward in that case.
#[derive(Clone)]
pub struct TokenSet {
    /// The bearer access token; short-lived (~1h for Microsoft Graph).
    pub access_token: Secret,
    /// The refresh token, when the provider issued/rotated one this exchange.
    pub refresh_token: Option<Secret>,
    /// The absolute instant the access token expires (response `now + expires_in`).
    pub expires_at: OffsetDateTime,
    /// The scopes actually granted (may differ from those requested).
    pub scope: String,
    /// The token type; `Bearer` in practice.
    pub token_type: String,
}

impl TokenSet {
    /// Whether the access token is expired (or within `skew` of expiring) at `now`;
    /// the cue to refresh before using it. A skew margin avoids sending a token that
    /// dies mid-request.
    #[must_use]
    pub fn is_expired(&self, now: OffsetDateTime, skew: Duration) -> bool {
        now + skew >= self.expires_at
    }
}

impl core::fmt::Debug for TokenSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // `Secret` already redacts, but spell the fields out so a struct dump can't
        // accidentally surface a token via a future field.
        f.debug_struct("TokenSet")
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_at", &self.expires_at)
            .field("scope", &self.scope)
            .field("token_type", &self.token_type)
            .finish()
    }
}

/// The token endpoint's success body.
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    #[serde(default)]
    scope: String,
    #[serde(default = "bearer")]
    token_type: String,
}

fn bearer() -> String {
    "Bearer".to_owned()
}

/// The token endpoint's error body (RFC 6749 §5.2).
#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
    error_description: Option<String>,
}

/// Exchanges an authorisation `code` (with its PKCE `verifier`) for tokens.
///
/// # Errors
///
/// Returns [`OAuthError::Endpoint`] if the provider rejects the exchange (e.g. an
/// expired/replayed code → `invalid_grant`), [`OAuthError::Transport`] on a network
/// failure, or [`OAuthError::Decode`] on an unparseable body.
pub async fn exchange_code(
    http: &reqwest::Client,
    provider: &OAuthProviderConfig,
    code: &str,
    pkce_verifier: &str,
    now: OffsetDateTime,
) -> Result<TokenSet, OAuthError> {
    let mut form = vec![
        ("client_id", provider.client_id.as_str()),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", provider.redirect_uri.as_str()),
        ("code_verifier", pkce_verifier),
    ];
    // A Google Desktop client's non-confidential secret, when present; Google's token endpoint
    // rejects the PKCE exchange without it. Absent for every true public client.
    if let Some(secret) = provider.client_secret.as_deref() {
        form.push(("client_secret", secret));
    }
    // RFC 8707: the resource the token is for. A server that issues resource-scoped tokens
    // rejects an exchange that omits it (`invalid_target`).
    if let Some(resource) = provider.resource.as_deref() {
        form.push(("resource", resource));
    }
    post_token(http, provider, &form, now).await
}

/// Redeems a `refresh_token` for a fresh access token (and possibly a rotated refresh
/// token).
///
/// **No `scope` is sent, and that is the point.** RFC 6749 §6 requires the requested scope on a
/// refresh to be a subset of what was originally granted, and says an omitted one is treated as
/// equal to the original grant. Sending this build's *current* list therefore breaks every grant
/// issued before the list grew: the server answers `invalid_scope` and the account stops working
/// entirely: not just the feature the new scope was for. Omitting it is the only value that
/// cannot go stale, and it costs nothing, because what a refresh is for is a new access token for
/// the scopes the person already consented to.
///
/// What this build wants and what the grant actually carries can therefore differ, which is a
/// state a caller has to handle rather than prevent: [`TokenSet::scope`] names what was issued,
/// and [`crate::GrantedScopes`] compares the two.
///
/// # Errors
///
/// Returns [`OAuthError::Endpoint`] if the provider rejects the refresh (a revoked or
/// expired refresh token → `invalid_grant`, the signal to re-authenticate),
/// [`OAuthError::Transport`] on a network failure, or [`OAuthError::Decode`] on an
/// unparseable body.
pub async fn refresh(
    http: &reqwest::Client,
    provider: &OAuthProviderConfig,
    refresh_token: &str,
    now: OffsetDateTime,
) -> Result<TokenSet, OAuthError> {
    let mut form = vec![
        ("client_id", provider.client_id.as_str()),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];
    // Same non-confidential Google Desktop secret as the code exchange: the refresh grant needs
    // it too, or the account would break at the first token refresh (~1h after setup).
    if let Some(secret) = provider.client_secret.as_deref() {
        form.push(("client_secret", secret));
    }
    // …and the same RFC 8707 target. Sending it on the exchange but not here is the subtle way
    // to build an account that works for exactly one hour.
    if let Some(resource) = provider.resource.as_deref() {
        form.push(("resource", resource));
    }
    post_token(http, provider, &form, now).await
}

/// The shared POST + response classification for both grant types.
async fn post_token(
    http: &reqwest::Client,
    provider: &OAuthProviderConfig,
    form: &[(&str, &str)],
    now: OffsetDateTime,
) -> Result<TokenSet, OAuthError> {
    let resp = http
        .post(&provider.token_endpoint)
        .form(form)
        .send()
        .await
        .map_err(OAuthError::Transport)?;
    let status = resp.status();
    // NOT `Transport`. We have the status line, so the server's handler ran and the refresh
    // token we presented is spent: a rotated replacement was in the body that just died on
    // us. The caller must not re-present the old one; see `OAuthError::reach`.
    let body = resp.text().await.map_err(OAuthError::ResponseLost)?;
    if !status.is_success() {
        // A well-formed OAuth error body names the reason; otherwise surface the raw text.
        return Err(match serde_json::from_str::<ErrorResponse>(&body) {
            Ok(err) => OAuthError::Endpoint {
                error: err.error,
                description: err.error_description,
            },
            Err(_) => OAuthError::Endpoint {
                error: format!("http {}", status.as_u16()),
                description: Some(body),
            },
        });
    }
    let parsed: TokenResponse =
        serde_json::from_str(&body).map_err(|err| OAuthError::Decode(err.to_string()))?;
    Ok(TokenSet {
        access_token: Secret::new(parsed.access_token),
        refresh_token: parsed.refresh_token.map(Secret::new),
        expires_at: now + Duration::seconds(parsed.expires_in),
        scope: parsed.scope,
        token_type: parsed.token_type,
    })
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
