//! [`RefreshingJmapProvider`]: a JMAP provider that keeps itself on a live access token.
//!
//! # Why a wrapper is needed at all
//!
//! An OAuth access token lives about an hour; an app session lives much longer. The engine's
//! [`JmapProvider`] is built once with a fixed credential and has no way to learn a new one,
//! so a provider connected at launch would start `401`ing after an hour and every later sync
//! would fail. Minting a token at connect time is not enough; connect happens once.
//!
//! So this wraps the delegate and mints the token **before each call**, rebuilding the
//! delegate only when the token actually changed (roughly hourly). The rebuild is not free
//! for JMAP the way it is for Graph (`JmapProvider::connect` re-runs session discovery) but
//! at once an hour that is immaterial, and caching by token keeps the reqwest connection pool
//! warm in between. This is the same shape as `RefreshingGmailProvider`, and it is why the
//! engine needs no OAuth code of its own.
//!
//! The `impl Provider` itself lives in the sibling `refreshing_provider` module, to keep both
//! files under the 500-line cap.

use std::sync::{Arc, Mutex};

use engine_provider::{Capabilities, Provider, ProviderError, ProviderResult};
use engine_tls::TlsClientConfig;
use provider_jmap::{Credentials, JmapConfig, JmapProvider};

use crate::{AccountError, GraphTokenSource, connect_log::connect_logger, throttle::account_retry};

/// A [`Provider`](engine_provider::Provider) for an OAuth JMAP account: refreshes the access
/// token as needed and delegates to a [`JmapProvider`] built with it. Account-global; one
/// serves the whole account's mail *and* calendar, exactly like the non-OAuth JMAP path.
pub(crate) struct RefreshingJmapProvider {
    /// The JMAP server base URL every rebuilt delegate re-discovers its session from.
    base_url: String,
    /// The shared, self-refreshing token source. Shared with the account's calendar provider
    /// so one refresh serves both.
    tokens: Arc<GraphTokenSource>,
    /// The capabilities learned from the **initial** connect's session resource, so
    /// `connection_info` can answer synchronously (and before any delegate is rebuilt)
    /// without claiming a capability this server does not have.
    capabilities: Capabilities,
    /// The account's shared TLS policy, cloned into each rebuilt client.
    tls: TlsClientConfig,
    /// The built delegate, cached by the access token it was built with. Rebuilt only when a
    /// refresh produced a new token; never per request, which would re-run session discovery
    /// and open a fresh TLS connection every single call.
    cached: Mutex<Option<(String, Arc<JmapProvider>)>>,
}

impl core::fmt::Debug for RefreshingJmapProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never surface the token the cache is keyed on.
        f.debug_struct("RefreshingJmapProvider")
            .field("base_url", &self.base_url)
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

impl RefreshingJmapProvider {
    /// Wraps an already-connected `initial` delegate, which was built with `access_token` and
    /// whose session answered `capabilities`.
    ///
    /// Taking the first delegate rather than connecting lazily means the caller has already
    /// proved the grant works and read the server's real capabilities: so this never
    /// advertises mail or calendar support the session did not report, and never pays a
    /// second connect straight after the first.
    pub(crate) fn new(
        base_url: String,
        tokens: Arc<GraphTokenSource>,
        tls: TlsClientConfig,
        access_token: String,
        initial: JmapProvider,
        capabilities: Capabilities,
    ) -> Self {
        Self {
            base_url,
            tokens,
            capabilities,
            tls,
            cached: Mutex::new(Some((access_token, Arc::new(initial)))),
        }
    }

    /// The capabilities the initial session reported.
    pub(crate) fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    /// The live delegate's whole [`ConnectionInfo`], or `None` before one exists.
    ///
    /// The wrapper caps *capabilities* on purpose but must forward the **transport facts**
    /// inside this value; they describe the delegate's connection rather than anything this
    /// wrapper promises, and one of them (`concurrent_fetches`) paces callers.
    pub(crate) fn delegate_connection_info(&self) -> Option<engine_provider::ConnectionInfo> {
        self.cached
            .lock()
            .expect("jmap delegate mutex poisoned")
            .as_ref()
            .map(|(_, provider)| provider.connection_info())
    }

    /// The delegate's contact-write destination; `None` before any delegate exists.
    ///
    /// `ContactsProvider::contact_destination` is **synchronous**, so the contacts impl
    /// cannot mint a token and await a delegate the way its other methods do. It reads the
    /// cached one instead. Which address book a write lands in is a fact only the delegate
    /// holds (it learns it from the session), so this forwards rather than reconstructing
    /// it: a wrapper that guessed the book would send writes to the wrong collection.
    pub(crate) fn delegate_contact_destination(
        &self,
    ) -> Option<engine_provider::ContactDestination> {
        use engine_provider::ContactsProvider as _;

        self.cached
            .lock()
            .expect("jmap delegate mutex poisoned")
            .as_ref()
            .and_then(|(_, provider)| provider.contact_destination())
    }

    /// Returns the delegate, reusing the cached one while the access token is unchanged and
    /// rebuilding it (a fresh session discovery) only when a refresh produced a new token.
    ///
    /// # Errors
    ///
    /// Returns an authentication error when the refresh token is dead (the host must re-auth),
    /// or a retryable error for a transient refresh/connect failure.
    pub(crate) async fn delegate(&self) -> ProviderResult<Arc<JmapProvider>> {
        let token = self.tokens.access_token().await.map_err(|err| match err {
            AccountError::SigninRejected(detail) => ProviderError::authentication(detail),
            other => ProviderError::retryable(other.to_string()),
        })?;
        {
            let cache = self.cached.lock().expect("jmap delegate mutex poisoned");
            if let Some((cached_token, provider)) = cache.as_ref()
                && *cached_token == token
            {
                return Ok(Arc::clone(provider));
            }
        }
        log::debug!("jmap: access token changed; rebuilding the provider session");
        let config = JmapConfig::new(self.base_url.clone(), Credentials::bearer(token.clone()))
            .with_tls(self.tls.clone())
            .with_retry(account_retry())
            .with_connect_observer(connect_logger("jmap"));
        let provider = Arc::new(
            JmapProvider::connect(config)
                .await
                .map_err(|err| ProviderError::retryable(err.to_string()))?,
        );
        *self.cached.lock().expect("jmap delegate mutex poisoned") =
            Some((token, Arc::clone(&provider)));
        Ok(provider)
    }
}
