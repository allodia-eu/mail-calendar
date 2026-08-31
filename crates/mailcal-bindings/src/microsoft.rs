//! Microsoft 365 (OAuth) account setup over the FFI.
//!
//! The browser half of the flow is the host's (an OS auth session: the redirect capture
//! is platform-specific), so the FFI is two calls: [`begin_microsoft_login`] mints the
//! authorization URL + an opaque `pending` handle the host holds; then
//! [`MailcalApp::complete_microsoft_login`](crate::MailcalApp::complete_microsoft_login)
//! validates the redirect, exchanges the code, connects the account, and writes the grant to
//! the host's store itself through the shared
//! [`AccountCredentialStore`](crate::AccountCredentialStore): the same port a later rotation
//! takes, so the credential has exactly one way in and out. All types here mirror the
//! password-account setup in `setup.rs`.

use mailcal_account::{MicrosoftConfig, Secret};
use mailcal_oauth::{MICROSOFT_GRAPH_SCOPES, OAuthClient, OAuthProviderConfig};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::MailcalError;

/// The default tenant (both work and personal Microsoft accounts) when a host gives none.
const DEFAULT_TENANT: &str = "common";

/// What [`begin_microsoft_login`] returns: the authorization URL to open in the system
/// browser, and an opaque `pending` handle the host holds until the redirect comes back.
#[derive(uniffi::Record)]
pub struct MicrosoftLoginStart {
    /// The authorization URL to open in the platform auth session.
    pub authorization_url: String,
    /// An opaque handle (state + PKCE verifier + app params) to pass to
    /// `complete_microsoft_login`. Transient; hold it in memory only.
    pub pending: String,
}

/// The transient state carried between begin and complete: the host round-trips this as
/// the opaque `pending` handle. It carries the PKCE verifier, so it is never persisted.
#[derive(Serialize, Deserialize)]
struct PendingLogin {
    client_id: String,
    tenant: String,
    redirect_uri: String,
    scopes: Vec<String>,
    state: String,
    verifier: String,
}

/// Starts the Microsoft OAuth flow: builds the PKCE authorization URL for **this build's**
/// Microsoft client registration ([`mailcal_oauth::credentials`]) at `redirect_uri` and `tenant`
/// (`common` when `None`), requesting the full Graph scope set (mail read/write, send, and
/// calendar). `login_hint` (the address the user is connecting or **re-consenting**; from
/// autodetection, or from a reconnect-for-calendar / reconnect-to-send prompt) targets that
/// account so Microsoft doesn't offer a different signed-in one; pass `None` to let the user pick.
///
/// `redirect_uri` stays the host's: Microsoft registers it per platform against the host's own
/// bundle/package identity, so unlike Google's it cannot be derived from the client id.
///
/// Logs the (re-)request **privacy-safely**: the scopes asked for and whether it targets an
/// existing account, never the address: so a support log shows a permission re-request was
/// initiated and exactly which scopes it requested (the usual cause of a `403` is an old grant
/// that predates `Mail.ReadWrite`/`Mail.Send`, which this call re-requests).
///
/// A client reaches this only after [`oauth_routes`](crate::oauth_routes) said the route exists,
/// so the no-registration error below is a wiring bug rather than something a user can provoke.
///
/// # Errors
///
/// Returns [`MailcalError::Config`] if this build carries no Microsoft registration, if the
/// OAuth HTTP client cannot be built, or if the `pending` handle cannot be encoded.
#[uniffi::export(default(login_hint = None))]
pub fn begin_microsoft_login(
    tenant: Option<String>,
    redirect_uri: String,
    login_hint: Option<String>,
) -> Result<MicrosoftLoginStart, MailcalError> {
    let Some(client_id) = mailcal_oauth::credentials::microsoft_client_id() else {
        return Err(MailcalError::Config(
            "this build carries no Microsoft sign-in".to_owned(),
        ));
    };
    let tenant = tenant
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_TENANT.to_owned());
    log::info!(
        "oauth: starting Microsoft sign-in ({}); requesting {} Graph scope(s): [{}]",
        if login_hint.is_some() {
            "re-consent targeting an existing account"
        } else {
            "new account, account picker shown"
        },
        MICROSOFT_GRAPH_SCOPES.len(),
        MICROSOFT_GRAPH_SCOPES.join(", "),
    );
    let provider = OAuthProviderConfig::microsoft(
        client_id.clone(),
        &tenant,
        redirect_uri.clone(),
        MICROSOFT_GRAPH_SCOPES,
    );
    let client = OAuthClient::new(provider).map_err(|err| MailcalError::Config(err.to_string()))?;
    let request = client.begin(login_hint.as_deref());
    let pending = PendingLogin {
        client_id,
        tenant,
        redirect_uri,
        scopes: MICROSOFT_GRAPH_SCOPES
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
        state: request.state,
        verifier: request.pkce.verifier().to_owned(),
    };
    let pending =
        serde_json::to_string(&pending).map_err(|err| MailcalError::Config(err.to_string()))?;
    Ok(MicrosoftLoginStart {
        authorization_url: request.authorization_url,
        pending,
    })
}

/// The fruit of a completed flow: the account's config plus the fresh access token (and
/// its expiry) from the code exchange, so the token source can be seeded without an
/// immediate re-refresh.
pub(crate) struct MicrosoftAuthorized {
    pub(crate) config: MicrosoftConfig,
    pub(crate) access_token: String,
    pub(crate) expires_at: OffsetDateTime,
}

/// Validates the redirect, exchanges the authorization code for tokens, and looks up the
/// account's own address; everything needed to build a [`MicrosoftConfig`], without yet
/// touching the app. Driven on the bindings runtime by `complete_microsoft_login`.
///
/// # Errors
///
/// Returns [`MailcalError::Config`] if `pending` is malformed, or [`MailcalError::Connect`]
/// if the exchange fails, no refresh token was issued, or the `/me` lookup fails.
pub(crate) async fn authorize(
    pending: &str,
    callback_url: &str,
    now: OffsetDateTime,
) -> Result<MicrosoftAuthorized, MailcalError> {
    let pending: PendingLogin =
        serde_json::from_str(pending).map_err(|err| MailcalError::Config(err.to_string()))?;
    let scopes: Vec<&str> = pending.scopes.iter().map(String::as_str).collect();
    let provider = OAuthProviderConfig::microsoft(
        pending.client_id.clone(),
        &pending.tenant,
        pending.redirect_uri.clone(),
        &scopes,
    );
    let client = OAuthClient::new(provider).map_err(|err| MailcalError::Config(err.to_string()))?;
    let tokens = client
        .complete(callback_url, &pending.state, &pending.verifier, now)
        .await
        .map_err(|err| MailcalError::Connect(err.to_string()))?;
    // `offline_access` was requested, so a refresh token is mandatory; without it the
    // account would break an hour after setup.
    let refresh_token = tokens.refresh_token.ok_or_else(|| {
        MailcalError::Connect("Microsoft issued no refresh token (offline_access scope)".to_owned())
    })?;
    let access_token = tokens.access_token.expose().to_owned();
    let email = mailcal_account::fetch_primary_address(&access_token)
        .await
        .map_err(|err| MailcalError::Connect(err.to_string()))?;
    let config = MicrosoftConfig {
        email,
        client_id: pending.client_id,
        tenant: pending.tenant,
        redirect_uri: pending.redirect_uri,
        scopes: pending.scopes,
        refresh_token: Secret::new(refresh_token.expose().to_owned()),
    };
    Ok(MicrosoftAuthorized {
        config,
        access_token,
        expires_at: tokens.expires_at,
    })
}
