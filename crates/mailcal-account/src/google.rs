//! Building the Gmail mail provider a Google account syncs through: the Google parallel of
//! [`crate::graph`].
//!
//! The provider asks the account's token source for the token on each request, through the
//! engine's credential source, and so does the account's IMAP side (`imap`): one source, so a
//! token replaced after either half's refusal is the one both present next. The engine binds the
//! two halves only when they sign in as the same credential subject, the account's id.
//!
//! Unlike Graph (per-folder providers), **Gmail mail sync is account-global**: one provider
//! covers every label/message under [`SyncScope::GmailMessages`], so there is a single mail
//! provider per account and no folder fan-out (the JMAP shape, not the IMAP/Graph one).
//!
//! The shared token source is [`GraphTokenSource`]; provider-neutral despite its name (it is
//! an `OAuthClient` + refresh token + rotation sink), so a Google account reuses it exactly as
//! a Microsoft one does, built from this account's [`GoogleConfig`]'s Google endpoints. It keeps
//! the default `permits`, so the token may go to every host Google's adapters address:
//! `www.googleapis.com`, `gmail.googleapis.com`, `people.googleapis.com`, `meet.googleapis.com`
//! and `imap.gmail.com`.

use std::sync::{Arc, Mutex};

use engine_api::{EmailAddress, RetryConfig, UserAgent};
use engine_core::{ids::AccountId, time::CalendarDate};
use engine_provider::{
    Capabilities, CredentialSource, IdentityControls, Provider, ProviderError, ProviderResult,
    ReportControls, ReportEvidence, ReportVerdicts,
};
use engine_tls::TlsClientConfig;
use mailcal_oauth::OAuthClient;
use provider_google::{GmailProvider, GoogleClient, GoogleError};
use time::Date;

mod calendar;
mod config;
mod contacts;
mod imap;
mod mail_provider;

pub use calendar::connect_google_calendar_provider;
pub use config::{GoogleConfig, fetch_google_primary_address, load_google_str};
pub use contacts::connect_google_contact_providers;
pub use imap::{GmailMailRoute, gmail_mail_route, on_gmail_mail_route_change};

use crate::{
    AccountError, GmailKeywordNaming, GraphTokenSource, TokenSink,
    shared_client::{SharedClient, lost_its_connection, should_reconnect},
    tls::tls_with,
};

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
        &config.email,
        config.refresh_token.expose().to_owned(),
        sink,
        "google",
        origin,
    ))
}

/// The client a Google provider forwards through, each request asking `tokens` for the token.
pub(crate) fn account_client(
    tokens: Arc<GraphTokenSource>,
    tls: TlsClientConfig,
) -> SharedClient<GoogleClient> {
    client_connecting(tokens, tls, GoogleClient::connect_with_source)
}

/// [`account_client`], each client connected by `connect`: Google's APIs, or a test's server.
pub(crate) fn client_connecting(
    tokens: Arc<GraphTokenSource>,
    tls: TlsClientConfig,
    connect: impl Fn(
        Arc<dyn CredentialSource>,
        &TlsClientConfig,
        &RetryConfig,
        &UserAgent,
    ) -> Result<GoogleClient, GoogleError>
    + Send
    + Sync
    + 'static,
) -> SharedClient<GoogleClient> {
    SharedClient::new(connector(tokens, tls, connect))
}

/// What builds one Google client of the account: `connect` over `tokens`.
pub(crate) fn connector(
    tokens: Arc<GraphTokenSource>,
    tls: TlsClientConfig,
    connect: impl Fn(
        Arc<dyn CredentialSource>,
        &TlsClientConfig,
        &RetryConfig,
        &UserAgent,
    ) -> Result<GoogleClient, GoogleError>
    + Send
    + Sync
    + 'static,
) -> impl Fn() -> ProviderResult<GoogleClient> + Send + Sync + 'static {
    move || {
        connect(
            Arc::clone(&tokens) as Arc<dyn CredentialSource>,
            &tls,
            &tokens.retry(),
            &crate::user_agent(),
        )
        .map_err(ProviderError::from)
    }
}

/// A delegate the wrapper built, and what it was built on.
#[derive(Debug)]
pub(super) struct BuiltDelegate {
    /// The generation of the client it was built on.
    pub(super) generation: u64,
    /// Whether it names the drafted-reply keyword.
    named: bool,
    pub(super) provider: Arc<GmailProvider>,
}

/// The delegate a cache holds, if any.
type Built = Option<BuiltDelegate>;

/// A [`Provider`] that forwards to a [`GmailProvider`] and replaces its client once after a
/// request fails on a dead connection. Account-global (no folder binding); one serves the whole
/// account's mail. Internal: the account layer hands callers `Box<dyn Provider>` from
/// [`connect_google_mail_providers`] / [`connect_google_folder`].
#[derive(Debug)]
pub(crate) struct ReconnectingGmailProvider {
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
    /// The client both delegates are built on.
    client: SharedClient<GoogleClient>,
    /// The delegate the writes, sends and label reads go through, over the API.
    cached: Mutex<Built>,
    /// The delegate the mail sync and body reads go through: [`cached`](Self::cached)'s, with
    /// the account's IMAP side attached when it has one. Kept apart so that IMAP being out of
    /// reach fails only the reads, never a send or a write, which go over the API.
    cached_reader: Mutex<Built>,
    /// The account's mail over IMAP, shared by every provider of the account (`imap`).
    imap: Arc<imap::GmailImapSlot>,
    /// Whether the delegate names the drafted-reply keyword; a delegate built before it turned on
    /// is built again.
    naming: GmailKeywordNaming,
}

impl ReconnectingGmailProvider {
    /// Binds a Gmail provider (windowed to `since`) asking `tokens`.
    #[must_use]
    pub(crate) fn new(
        tokens: Arc<GraphTokenSource>,
        since: Option<Date>,
        tls: TlsClientConfig,
        address: &EmailAddress,
        naming: GmailKeywordNaming,
    ) -> Self {
        Self {
            naming,
            imap: imap::GmailImapSlot::shared(&address.email, &tokens, &tls),
            client: account_client(tokens, tls),
            cached: Mutex::new(None),
            cached_reader: Mutex::new(None),
            capabilities: Capabilities::none()
                .with_mail()
                .with_mail_writes()
                // Forwarded by this wrapper's `MailboxWrites`.
                .with_mailbox_writes()
                // Forwarded by this wrapper's `Provider`, and **without** phishing:
                // Gmail's label set has no phishing member, so asking for that verdict is a
                // hard error rather than a near-enough filing under spam.
                .with_mail_report(ReportControls {
                    verdicts: ReportVerdicts::without_phishing(),
                    evidence: ReportEvidence::Convention,
                })
                // Forwarded by this wrapper's `Provider`: `put_draft` and `delete_draft`.
                .with_mail_drafts()
                .with_submission()
                // This wrapper reports its own capabilities rather than the delegate's, so a
                // flag omitted here is a flag the account does not have; however loudly the
                // adapter underneath advertises it. `submit_email` forwards the whole `Draft`,
                // Gmail submits assembled RFC 5322 bytes, and so the `method=` parameter an
                // iMIP body part needs (RFC 6047 §2.4) survives.
                .with_scheduling_submission()
                // A delegate built while the install names the drafted-reply label is what makes
                // Gmail keep a Sent copy's keyword; the receipt saying which it kept is forwarded.
                .with_sent_copy_keywords()
                // The same names keep a stored draft's keyword, forwarded with `put_draft`.
                .with_draft_keywords()
                // Forwarded below. Gmail's send-as settings are the account holder's to
                // change, so the name is `Writable` rather than a directory's to hand down.
                .with_sender_identities(IdentityControls::Writable),
            since,
        }
    }

    /// The delegate that reads over the API, and the generation of the client it was built on;
    /// [`reading_delegate`](Self::reading_delegate) is the one the mail sync uses.
    pub(super) async fn delegate(&self) -> ProviderResult<(u64, Arc<GmailProvider>)> {
        self.built(&self.cached, false).await
    }

    /// The delegate the mail sync and body reads go through: over IMAP when the account has
    /// that side, else over the API. Fails, to be tried again, while IMAP cannot be reached. Beside
    /// it, the generation of the client it was built on, which a read over IMAP uses too: a pass
    /// reads the account's labels over the API before it reads mail over IMAP.
    pub(super) async fn reading_delegate(&self) -> ProviderResult<(u64, Arc<GmailProvider>)> {
        self.built(&self.cached_reader, true).await
    }

    /// The delegate in `cache`, built again when the client was replaced or the keyword naming
    /// changed since, and the generation of the client it was built on.
    ///
    /// # Errors
    ///
    /// `InvalidState` when the IMAP side signs in as another credential subject than the client,
    /// which is a fault in how the two were built and is never answered by reading over the API.
    async fn built(
        &self,
        cache: &Mutex<Built>,
        reads: bool,
    ) -> ProviderResult<(u64, Arc<GmailProvider>)> {
        let (generation, client) = self.client.current()?;
        let named = self.naming.is_on();
        {
            let cached = cache.lock().expect("gmail delegate mutex poisoned");
            if let Some(built) = cached.as_ref()
                && built.generation >= generation
                && built.named == named
            {
                return Ok((built.generation, Arc::clone(&built.provider)));
            }
        }
        let mut gmail = GmailProvider::new(client).with_keyword_names(if named {
            crate::drafted_reply_keyword_names()
        } else {
            Vec::new()
        });
        if reads && let Some(reader) = self.imap.reader().await? {
            gmail = gmail.with_imap(reader)?;
        }
        if let Some(date) = self.since.and_then(calendar_date) {
            gmail = gmail.with_since(date);
        }
        let built = BuiltDelegate {
            generation,
            named,
            provider: Arc::new(gmail),
        };
        let answer = (generation, Arc::clone(&built.provider));
        let mut cached = cache.lock().expect("gmail delegate mutex poisoned");
        if cached
            .as_ref()
            .is_none_or(|stored| stored.generation <= generation)
        {
            *cached = Some(built);
        }
        Ok(answer)
    }

    /// Runs `call` on a delegate, the reading one when `reads`, and once more on a new client when
    /// the first attempt failed on a dead connection
    /// ([`should_reconnect`](crate::shared_client::should_reconnect)). A read over IMAP replaces
    /// the client too: the connection that died may be the API's, which a pass reads the labels
    /// over, and a new client costs nothing more; the engine replaces IMAP's own connections. For
    /// a request that repeats safely.
    pub(super) async fn retrying<T, F, Fut>(&self, reads: bool, call: F) -> ProviderResult<T>
    where
        F: Fn(Arc<GmailProvider>) -> Fut,
        Fut: Future<Output = ProviderResult<T>>,
    {
        let cache = if reads {
            &self.cached_reader
        } else {
            &self.cached
        };
        let (generation, provider) = self.built(cache, reads).await?;
        match call(provider).await {
            Err(err) if should_reconnect(&err) => {
                self.drop_client(generation);
                call(self.built(cache, reads).await?.1).await
            }
            result => result,
        }
    }

    /// Runs `call` once on the API delegate: a request that may have been applied before the
    /// connection died. A dead connection still drops the client for the next request.
    pub(super) async fn once<T, Fut>(
        &self,
        call: impl FnOnce(Arc<GmailProvider>) -> Fut,
    ) -> ProviderResult<T>
    where
        Fut: Future<Output = ProviderResult<T>>,
    {
        let (generation, provider) = self.delegate().await?;
        call(provider).await.inspect_err(|err| {
            if lost_its_connection(err) {
                self.drop_client(generation);
            }
        })
    }

    /// Drops the client of `generation`, which a request on it found dead.
    pub(super) fn drop_client(&self, generation: u64) {
        self.client.drop_dead(generation);
    }
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
    address: &EmailAddress,
    naming: GmailKeywordNaming,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    let tls = tls_with(&[])?;
    // Cheap credential probe: refresh once so a dead refresh token fails here.
    tokens.access_token().await?;
    Ok(vec![Box::new(ReconnectingGmailProvider::new(
        tokens, since, tls, address, naming,
    ))])
}

/// Builds an on-demand Gmail provider for a Google account, asking its `tokens`. Gmail's
/// provider is account-wide (one scope covers every label), so (like JMAP) the returned
/// provider serves any folder the host opens; there is no per-folder binding. Sync: nothing is
/// sent before the first call.
///
/// # Errors
///
/// Returns [`AccountError::Tls`] if the account TLS policy can't be built.
pub fn connect_google_folder(
    tokens: Arc<GraphTokenSource>,
    since: Option<Date>,
    address: &EmailAddress,
    naming: GmailKeywordNaming,
) -> Result<Box<dyn Provider>, AccountError> {
    let tls = tls_with(&[])?;
    Ok(Box::new(ReconnectingGmailProvider::new(
        tokens, since, tls, address, naming,
    )))
}

#[cfg(test)]
mod tests {
    use engine_core::sync::SyncScope;
    use mailcal_oauth::{GOOGLE_SCOPES, OAuthClient, OAuthProviderConfig};

    use super::*;

    fn alice() -> EmailAddress {
        EmailAddress::new("alice@example.com")
    }

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
            "alice@gmail.com",
            "refresh".to_owned(),
            None,
            "google",
            crate::CredentialOrigin::FreshSignIn,
        )
    }

    /// The same gap as the Graph wrapper's: Gmail's adapter saves and removes drafts, and this
    /// wrapper neither advertised nor forwarded either verb, so every save was refused.
    #[tokio::test]
    async fn drafts_are_advertised_and_forwarded_to_gmail() {
        use crate::graph::test_support::{
            assert_drafts_reach_the_delegate, dead_grant_token_endpoint, source_at,
        };

        let (endpoint, _hits) = dead_grant_token_endpoint();
        let provider = connect_google_folder(
            source_at(endpoint, None),
            None,
            &alice(),
            GmailKeywordNaming::default(),
        )
        .unwrap();
        assert_drafts_reach_the_delegate(provider.as_ref()).await;
    }

    #[test]
    fn connect_google_folder_binds_a_writable_account_global_mail_provider() {
        let source = google_source();
        let account = AccountId::try_from("alice@gmail.com@mail.google.com").unwrap();
        let provider =
            connect_google_folder(source, None, &alice(), GmailKeywordNaming::default()).unwrap();
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
        assert!(provider.connection_info().capabilities.sent_copy_keywords());
        assert!(provider.connection_info().capabilities.draft_keywords());
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
