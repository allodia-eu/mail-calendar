//! Google (Gmail + Google Calendar) OAuth account setup over the FFI: the sibling of
//! [`crate::microsoft`].
//!
//! The browser half of the flow is the host's (an OS auth session / loopback listener; the
//! redirect capture is platform-specific), so the FFI is two calls: [`begin_google_login`]
//! mints the authorization URL + an opaque `pending` handle the host holds; then
//! [`MailcalApp::complete_google_login`](crate::MailcalApp::complete_google_login) validates the
//! redirect, exchanges the code, connects the account, and writes the grant to the host's store
//! itself through the shared [`AccountCredentialStore`](crate::AccountCredentialStore): the same
//! port a later rotation takes. Because Google gates access on **Early Access** allow-listed
//! test users during the unverified period, a client shows the Early Access notice + sign-up
//! confirmation *before* invoking this: the core is unaware of that gate.
//!
//! Google refresh tokens are long-lived and do **not** rotate on a refresh grant (unlike
//! Microsoft's), so re-persisting a rotation through the shared
//! [`AccountCredentialStore`](crate::AccountCredentialStore) is a robustness backstop that in
//! practice rarely fires; the shared token sink (`crate::token_sink`) handles every provider.

use mailcal_account::{GoogleConfig, Secret};
use mailcal_oauth::{GOOGLE_SCOPES, OAuthClient, OAuthProviderConfig};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::MailcalError;

/// What [`begin_google_login`] returns: the authorization URL to open in the system browser (or
/// loopback flow), and an opaque `pending` handle the host holds until the redirect comes back.
#[derive(uniffi::Record)]
pub struct GoogleLoginStart {
    /// The authorization URL to open in the platform auth session / default browser.
    pub authorization_url: String,
    /// An opaque handle (state + PKCE verifier + app params) to pass to `complete_google_login`.
    /// Transient; hold it in memory only.
    pub pending: String,
}

/// The transient state carried between begin and complete: the host round-trips this as the
/// opaque `pending` handle. It carries the PKCE verifier, so it is never persisted.
#[derive(Serialize, Deserialize)]
struct PendingGoogleLogin {
    client_id: String,
    /// The non-confidential Google Desktop client secret (macOS/Windows loopback), or `None`
    /// for an iOS/Android client. Round-tripped so `complete` can send it on the exchange.
    #[serde(default)]
    client_secret: Option<String>,
    redirect_uri: String,
    scopes: Vec<String>,
    state: String,
    verifier: String,
}

/// Starts the Google OAuth flow: builds the PKCE authorization URL for **this build's** Google
/// client registration ([`mailcal_oauth::credentials`]), requesting the full Gmail + Google
/// Calendar scopes with `access_type=offline`.
///
/// `redirect_uri` stays the host's because only the host knows it: the mobile client types
/// redirect to the fixed custom scheme [`oauth_routes`](crate::oauth_routes) hands back, while
/// the Desktop client redirects to a loopback port the host binds per flow. `login_hint` (the
/// address the user is connecting, e.g. from autodetection) targets that account; pass `None` to
/// let the user pick.
///
/// A client reaches this only after [`oauth_routes`](crate::oauth_routes) said the route exists,
/// so the no-registration error below is a wiring bug rather than something a user can provoke.
///
/// # Errors
///
/// Returns [`MailcalError::Config`] if this build carries no Google registration, if the OAuth
/// HTTP client cannot be built, or if the `pending` handle cannot be encoded.
#[uniffi::export(default(login_hint = None))]
pub fn begin_google_login(
    redirect_uri: String,
    login_hint: Option<String>,
) -> Result<GoogleLoginStart, MailcalError> {
    let Some(registration) = mailcal_oauth::credentials::google() else {
        return Err(MailcalError::Config(
            "this build carries no Google sign-in".to_owned(),
        ));
    };
    let (client_id, client_secret) = (registration.client_id, registration.client_secret);
    log::info!(
        "google: begin sign-in (client_secret {}, login_hint {})",
        if client_secret.is_some() {
            "present"
        } else {
            "absent"
        },
        if login_hint.is_some() {
            "present"
        } else {
            "absent"
        },
    );
    let provider = OAuthProviderConfig::google(
        client_id.clone(),
        client_secret.clone(),
        redirect_uri.clone(),
        GOOGLE_SCOPES,
    );
    let client = OAuthClient::new(provider).map_err(|err| MailcalError::Config(err.to_string()))?;
    let request = client.begin(login_hint.as_deref());
    let pending = PendingGoogleLogin {
        client_id,
        client_secret,
        redirect_uri,
        scopes: GOOGLE_SCOPES.iter().map(|s| (*s).to_owned()).collect(),
        state: request.state,
        verifier: request.pkce.verifier().to_owned(),
    };
    let pending =
        serde_json::to_string(&pending).map_err(|err| MailcalError::Config(err.to_string()))?;
    Ok(GoogleLoginStart {
        authorization_url: request.authorization_url,
        pending,
    })
}

/// The fruit of a completed flow: the account's config plus the fresh access token (and its
/// expiry) from the code exchange, so the token source can be seeded without an immediate
/// re-refresh.
pub(crate) struct GoogleAuthorized {
    pub(crate) config: GoogleConfig,
    pub(crate) access_token: String,
    pub(crate) expires_at: OffsetDateTime,
}

/// Validates the redirect, exchanges the authorization code for tokens, and looks up the
/// account's own address (Gmail profile); everything needed to build a [`GoogleConfig`],
/// without yet touching the app. Driven on the bindings runtime by `complete_google_login`.
///
/// # Errors
///
/// Returns [`MailcalError::Config`] if `pending` is malformed, or [`MailcalError::Connect`] if
/// the exchange fails, no refresh token was issued, or the profile lookup fails.
pub(crate) async fn authorize(
    pending: &str,
    callback_url: &str,
    now: OffsetDateTime,
) -> Result<GoogleAuthorized, MailcalError> {
    let pending: PendingGoogleLogin =
        serde_json::from_str(pending).map_err(|err| MailcalError::Config(err.to_string()))?;
    let scopes: Vec<&str> = pending.scopes.iter().map(String::as_str).collect();
    let provider = OAuthProviderConfig::google(
        pending.client_id.clone(),
        pending.client_secret.clone(),
        pending.redirect_uri.clone(),
        &scopes,
    );
    let client = OAuthClient::new(provider).map_err(|err| MailcalError::Config(err.to_string()))?;
    let tokens = client
        .complete(callback_url, &pending.state, &pending.verifier, now)
        .await
        .map_err(|err| MailcalError::Connect(err.to_string()))?;
    // `access_type=offline` + `prompt=consent` were requested, so a refresh token is mandatory;
    // without it the account would break an hour after setup.
    let refresh_token = tokens.refresh_token.ok_or_else(|| {
        MailcalError::Connect("Google issued no refresh token (access_type=offline)".to_owned())
    })?;
    let access_token = tokens.access_token.expose().to_owned();
    let email = mailcal_account::fetch_google_primary_address(&access_token)
        .await
        .map_err(|err| MailcalError::Connect(err.to_string()))?;
    let config = GoogleConfig {
        email,
        client_id: pending.client_id,
        client_secret: pending.client_secret,
        redirect_uri: pending.redirect_uri,
        scopes: pending.scopes,
        refresh_token: Secret::new(refresh_token.expose().to_owned()),
    };
    Ok(GoogleAuthorized {
        config,
        access_token,
        expires_at: tokens.expires_at,
    })
}
