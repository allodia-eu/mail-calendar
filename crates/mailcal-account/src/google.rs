//! Building the Gmail mail provider a Google account syncs through, with automatic OAuth
//! token refresh: the Google parallel of [`crate::graph`].
//!
//! The engine's [`GmailProvider`] takes a **static** bearer token, but Google access tokens
//! live ~1 hour; far shorter than an app session. So this module wraps it in a
//! [`RefreshingGmailProvider`] that mints a fresh access token (refreshing from the stored
//! refresh token when the cached one is stale) **before each network call**, then delegates to
//! a freshly built `GmailProvider`. Google is stateless HTTP (no socket to keep alive) so
//! rebuilding the client per token is cheap; the built delegate is cached by its access token
//! so the reqwest connection pool is reused until the token refreshes.
//!
//! Unlike Graph (per-folder providers), **Gmail mail sync is account-global**: one provider
//! covers every label/message under [`SyncScope::GmailMessages`], so there is a single mail
//! provider per account and no folder fan-out (the JMAP shape, not the IMAP/Graph one).
//!
//! The shared token source is [`GraphTokenSource`]; provider-neutral despite its name (it is
//! an `OAuthClient` + refresh token + rotation sink), so a Google account reuses it exactly as
//! a Microsoft one does, built from this account's [`GoogleConfig`]'s Google endpoints.

use std::sync::{Arc, Mutex};

use engine_core::{error::FailureClass, ids::AccountId, time::CalendarDate};
use engine_provider::{
    Capabilities, Provider, ProviderError, ProviderResult, ReportControls, ReportEvidence,
    ReportVerdicts,
};
use engine_tls::TlsClientConfig;
use mailcal_oauth::OAuthClient;
use provider_google::{GmailProvider, GoogleClient};
use time::Date;

mod calendar;
mod config;
mod mail_provider;

pub use calendar::connect_google_calendar_providers;
pub use config::{GoogleConfig, fetch_google_primary_address, load_google_str};

use crate::{AccountError, GraphTokenSource, TokenSink, throttle::account_retry, tls::account_tls};

/// Builds the shared, self-refreshing token source for a Google `config`: the Google parallel
/// of [`GraphTokenSource::new`](crate::GraphTokenSource) (which takes a `MicrosoftConfig`). It
/// reuses the same provider-neutral token source, built from the account's Google OAuth
/// endpoints; `sink` (optional) receives a rotated refresh token for re-persistence (rare for
/// Google, which does not rotate on a refresh grant).
///
/// # Errors
///
/// Returns [`AccountError::Google`] if the OAuth HTTP client cannot be built.
pub fn google_token_source(
    config: &GoogleConfig,
    account: AccountId,
    sink: Option<Arc<dyn TokenSink>>,
    origin: crate::CredentialOrigin,
) -> Result<Arc<GraphTokenSource>, AccountError> {
    let oauth = OAuthClient::new(config.provider_config())
        .map_err(|err| AccountError::Google(err.to_string()))?;
    Ok(GraphTokenSource::from_parts(
        oauth,
        account,
        config.refresh_token.expose().to_owned(),
        sink,
        "google",
        origin,
    ))
}

/// A [`Provider`] that refreshes its Google access token before every network call and
/// delegates to a freshly built [`GmailProvider`]. Account-global (no folder binding); one
/// serves the whole account's mail. Internal: the account layer hands callers
/// `Box<dyn Provider>` from [`connect_google_mail_providers`] / [`connect_google_folder`].
#[derive(Debug)]
pub(crate) struct RefreshingGmailProvider {
    /// The shared, self-refreshing token source (also shared with the account's calendar
    /// provider so one refresh serves both).
    tokens: Arc<GraphTokenSource>,
    /// The capabilities the wrapper reports; mail read/sync, **mail writes, and submission**,
    /// each of which it forwards to the delegate below. It advertises exactly what it forwards:
    /// a capability claimed but not forwarded falls through to the `Provider` trait's rejecting
    /// default, which is how Gmail archive/delete/mark-read/send silently failed with
    /// `InvalidState: provider does not support mail writes` while the delegate could do all of
    /// it.
    capabilities: Capabilities,
    /// The sync-depth cutoff, applied to the built delegate so the initial snapshot is windowed
    /// to recent mail (`None` syncs the whole account).
    since: Option<Date>,
    /// The account's shared TLS policy, cloned into each rebuilt Google client.
    tls: TlsClientConfig,
    /// The built delegate, cached by the access token it was built with, so its reqwest
    /// connection pool is reused across requests; rebuilt only when the token refreshes.
    cached: Mutex<Option<(String, Arc<GmailProvider>)>>,
}

impl RefreshingGmailProvider {
    /// Binds a refreshing Gmail provider (windowed to `since`) sharing `tokens`.
    #[must_use]
    pub(crate) fn new(
        tokens: Arc<GraphTokenSource>,
        since: Option<Date>,
        tls: TlsClientConfig,
    ) -> Self {
        Self {
            tokens,
            capabilities: Capabilities::none()
                .with_mail()
                .with_mail_writes()
                // Forwarded by this wrapper's `Provider`, and **without** phishing:
                // Gmail's label set has no phishing member, so asking for that verdict is a
                // hard error rather than a near-enough filing under spam.
                .with_mail_report(ReportControls {
                    verdicts: ReportVerdicts::without_phishing(),
                    evidence: ReportEvidence::Convention,
                })
                .with_submission()
                // This wrapper reports its own capabilities rather than the delegate's, so a
                // flag omitted here is a flag the account does not have; however loudly the
                // adapter underneath advertises it. `submit_email` forwards the whole `Draft`,
                // Gmail submits assembled RFC 5322 bytes, and so the `method=` parameter an
                // iMIP body part needs (RFC 6047 §2.4) survives.
                .with_scheduling_submission(),
            since,
            tls,
            cached: Mutex::new(None),
        }
    }

    /// Returns the delegate `GmailProvider`, reusing the cached one while the access token is
    /// unchanged and rebuilding it only when a refresh produced a new token.
    async fn delegate(&self) -> ProviderResult<Arc<GmailProvider>> {
        let token = self.tokens.access_token().await.map_err(|err| match err {
            AccountError::SigninRejected(detail) => ProviderError::authentication(detail),
            other => ProviderError::retryable(other.to_string()),
        })?;
        {
            let cache = self.cached.lock().expect("gmail delegate mutex poisoned");
            if let Some((cached_token, provider)) = cache.as_ref()
                && *cached_token == token
            {
                return Ok(Arc::clone(provider));
            }
        }
        let client = GoogleClient::connect(token.clone(), &self.tls, &account_retry())
            .map_err(ProviderError::from)?;
        let mut gmail = GmailProvider::new(client);
        if let Some(date) = self.since.and_then(calendar_date) {
            gmail = gmail.with_since(date);
        }
        let provider = Arc::new(gmail);
        *self.cached.lock().expect("gmail delegate mutex poisoned") =
            Some((token, Arc::clone(&provider)));
        Ok(provider)
    }

    /// Drops the cached delegate so the next [`delegate`](Self::delegate) rebuilds a fresh
    /// client + connection pool; used after a retryable transport failure whose keep-alive
    /// socket is presumed dead (the token is unchanged, so this only rebuilds the client).
    fn invalidate_delegate(&self) {
        *self.cached.lock().expect("gmail delegate mutex poisoned") = None;
    }
}

/// Whether a failed call warrants dropping the cached delegate and rebuilding it: a retryable
/// **transport** error (a stale keep-alive socket after sleep) a fresh client may clear.
pub(super) fn should_reconnect(err: &ProviderError) -> bool {
    err.class() == FailureClass::Retryable
}

/// Converts a `time::Date` sync-depth cutoff into the engine's `CalendarDate`.
pub(super) fn calendar_date(date: Date) -> Option<CalendarDate> {
    CalendarDate::new(date.year(), u8::from(date.month()), date.day()).ok()
}

/// Connects the Gmail mail provider a Google account syncs: **one** account-global provider
/// (Gmail's message scope is account-wide: the JMAP shape, unlike Graph's per-folder fan-out).
/// Probes the credential up front (like the Graph folder-list enumeration) so a revoked
/// refresh token surfaces as a connect failure now, not on the first background sync.
///
/// # Errors
///
/// Returns [`AccountError`] if the TLS policy can't be built or the initial token refresh fails
/// (a revoked/expired refresh token → [`AccountError::SigninRejected`], the re-auth signal).
pub async fn connect_google_mail_providers(
    tokens: Arc<GraphTokenSource>,
    since: Option<Date>,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    let tls = account_tls()?;
    // Cheap credential probe: refresh once so a dead refresh token fails here.
    let _ = tokens.access_token().await?;
    Ok(vec![Box::new(RefreshingGmailProvider::new(
        tokens, since, tls,
    ))])
}

/// Builds an on-demand Gmail provider for a Google account, sharing its `tokens`. Gmail's
/// provider is account-wide (one scope covers every label), so (like JMAP) the returned
/// provider serves any folder the host opens; there is no per-folder binding. Sync (the token
/// is fetched lazily on the first call).
///
/// # Errors
///
/// Returns [`AccountError::Tls`] if the account TLS policy can't be built.
pub fn connect_google_folder(
    tokens: Arc<GraphTokenSource>,
    since: Option<Date>,
) -> Result<Box<dyn Provider>, AccountError> {
    let tls = account_tls()?;
    Ok(Box::new(RefreshingGmailProvider::new(tokens, since, tls)))
}

#[cfg(test)]
mod tests {
    use engine_core::sync::SyncScope;
    use mailcal_oauth::{GOOGLE_SCOPES, OAuthClient, OAuthProviderConfig};

    use super::*;

    /// A Google token source pointed at a dummy provider: no network is performed by the
    /// constructors under test (`connect_google_folder` only builds the wrapper).
    fn google_source() -> Arc<GraphTokenSource> {
        let provider = OAuthProviderConfig::google(
            "google-client",
            None,
            "com.googleusercontent.apps.google-client:/oauth2redirect",
            GOOGLE_SCOPES,
        );
        GraphTokenSource::from_parts(
            OAuthClient::new(provider).unwrap(),
            AccountId::try_from("alice@gmail.com@mail.google.com").unwrap(),
            "refresh".to_owned(),
            None,
            "google",
            crate::CredentialOrigin::FreshSignIn,
        )
    }

    #[test]
    fn a_retryable_transport_error_reconnects_but_a_rate_limit_or_auth_does_not() {
        assert!(should_reconnect(&ProviderError::retryable("broken pipe")));
        assert!(!should_reconnect(&ProviderError::new(
            FailureClass::RateLimited,
            "429",
        )));
        assert!(!should_reconnect(&ProviderError::authentication("401")));
    }

    #[test]
    fn connect_google_folder_binds_a_writable_account_global_mail_provider() {
        let source = google_source();
        let account = AccountId::try_from("alice@gmail.com@mail.google.com").unwrap();
        let provider = connect_google_folder(source, None).unwrap();
        // Mail read/sync, and the account-global message scope (no folder binding).
        assert!(provider.connection_info().capabilities.mail());
        assert!(!provider.connection_info().capabilities.calendars());
        // Writes + submission are forwarded, so they must be advertised. A capability this
        // wrapper does not claim is never attempted by the app, and one it claims but does
        // not forward hits the trait's rejecting default, which is how every Gmail archive,
        // delete, mark-read and send used to fail with "provider does not support mail
        // writes" even though the delegate implements all of them.
        assert!(provider.connection_info().capabilities.mail_writes());
        assert!(provider.connection_info().capabilities.submission());
        // Same rule, one capability further along: this wrapper reports its own set and never
        // the delegate's, so an iMIP capability missing here is missing from the account no
        // matter what Gmail advertises.
        assert!(
            provider
                .connection_info()
                .capabilities
                .scheduling_submission()
        );
        assert_eq!(
            provider.email_scope(&account),
            SyncScope::GmailMessages {
                account: account.clone(),
            }
        );
        assert_eq!(
            provider.mailbox_scope(&account),
            SyncScope::GmailLabelList { account }
        );
    }
}
