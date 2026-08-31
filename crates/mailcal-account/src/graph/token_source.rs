//! The shared, self-refreshing source of Microsoft Graph access tokens for one account.
//!
//! Graph access tokens live ~1 hour, far shorter than an app session, so every folder
//! provider shares one [`GraphTokenSource`] that mints a fresh access token (refreshing
//! from the stored refresh token when the cached one is stale) on demand. It also owns the
//! account's shared **concurrency gate**, so the folder providers throttle together against
//! Microsoft's per-mailbox concurrency limit, and reports a rotated refresh token to the
//! host via [`TokenSink`] to be re-persisted in the OS keystore.

use std::sync::Arc;

use async_trait::async_trait;
use engine_core::ids::AccountId;
use mailcal_oauth::{OAuthClient, TokenRequestReach};
use time::{Duration, OffsetDateTime};
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::{AccountError, MicrosoftConfig};

mod failure;
mod shared;

use failure::{
    DEAD_GRANT_COOLDOWN, FailureKind, MAYBE_PROCESSED_COOLDOWN, NOT_SENT_COOLDOWN, RefreshFailure,
};
pub use shared::CredentialOrigin;
use shared::{SharedCredential, credential_for};

/// Refresh the access token this long **before** it actually expires, so a token never
/// dies mid-request.
const REFRESH_SKEW: Duration = Duration::minutes(5);

/// The maximum number of **concurrent** Graph requests against one mailbox. Microsoft
/// throttles a mailbox at ~4 concurrent requests (`ApplicationThrottled` /
/// `MailboxConcurrency`), so a shared semaphore caps every one of the account's folder
/// providers to this; otherwise the eager role folders sync all at once and get 429ed.
const MAX_GRAPH_CONCURRENCY: usize = 4;

/// A host sink for a **rotated** refresh token: Microsoft may return a new refresh token
/// on each refresh and eventually invalidate the old one, so the host must overwrite the
/// stored config in its OS keystore. Implemented by the bindings over the platform secure
/// store; `None` (tests, or a host that hasn't wired it) keeps the rotation in memory only.
#[async_trait]
pub trait TokenSink: Send + Sync {
    /// Reports that `account`'s refresh token was rotated to `new_refresh_token`; the
    /// host re-serializes and re-persists that account's config.
    async fn refresh_token_rotated(&self, account: &AccountId, new_refresh_token: &str);
}

/// A shared, self-refreshing source of Graph access tokens for one account, and the
/// account's shared **concurrency gate**, so every folder provider throttles together
/// against Microsoft's per-mailbox concurrency limit.
pub struct GraphTokenSource {
    oauth: OAuthClient,
    account: AccountId,
    /// Which provider family this source serves (`graph` / `google` / `jmap`): the shared
    /// type is provider-neutral, and a refresh log line that cannot say *whose* token it is
    /// answers half the question. Safe to log: it names the protocol and nothing else.
    provider: &'static str,
    sink: Option<Arc<dyn TokenSink>>,
    /// The account's credential state and its refresh single-flight; **shared with every other
    /// token source for this account in this process**, not owned.
    ///
    /// Owning it was the bug. Without the single-flight, every provider that found the access
    /// token stale posted its own refresh with the *same* refresh token; a server that rotates
    /// then sees a **replay**, and one that ratchets; Fastmail answers `invalid_grant;
    /// ratchet or client_id mismatch`; revokes the entire grant. Owning the single-flight
    /// fixed that within one core and left it wide open between two, which a host produces
    /// routinely (see [`shared`]). Sharing the state closes both with the same mechanism.
    credential: Arc<SharedCredential>,
    /// Caps concurrent Graph requests for this mailbox (see [`MAX_GRAPH_CONCURRENCY`]).
    /// Deliberately **per source**: it is a politeness bound on one core's own fan-out, not a
    /// property of the credential.
    concurrency: Semaphore,
}

impl core::fmt::Debug for GraphTokenSource {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never surface either token.
        f.debug_struct("GraphTokenSource")
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}

impl GraphTokenSource {
    /// Builds a token source for `config`, minting access tokens from its stored refresh
    /// token. `sink` (optional) receives a rotated refresh token for re-persistence.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::Graph`] if the OAuth HTTP client cannot be built.
    pub fn new(
        config: &MicrosoftConfig,
        account: AccountId,
        sink: Option<Arc<dyn TokenSink>>,
        origin: CredentialOrigin,
    ) -> Result<Arc<Self>, AccountError> {
        let oauth = OAuthClient::new(config.provider_config())
            .map_err(|err| AccountError::Graph(err.to_string()))?;
        Ok(Self::from_parts(
            oauth,
            account,
            config.refresh_token.expose().to_owned(),
            sink,
            "graph",
            origin,
        ))
    }

    /// Builds a token source from an already-constructed [`OAuthClient`]: the seam
    /// offline tests use to point the refresh at a mock token endpoint.
    #[must_use]
    pub fn from_parts(
        oauth: OAuthClient,
        account: AccountId,
        refresh_token: String,
        sink: Option<Arc<dyn TokenSink>>,
        provider: &'static str,
        origin: CredentialOrigin,
    ) -> Arc<Self> {
        let credential = credential_for(&account, refresh_token, origin);
        Arc::new(Self {
            oauth,
            account,
            provider,
            sink,
            credential,
            concurrency: Semaphore::new(MAX_GRAPH_CONCURRENCY),
        })
    }

    /// Acquires a permit from the mailbox's shared concurrency gate; held for the
    /// duration of one Graph request so the account never exceeds
    /// [`MAX_GRAPH_CONCURRENCY`] concurrent requests.
    pub(super) async fn acquire(&self) -> SemaphorePermit<'_> {
        self.concurrency
            .acquire()
            .await
            .expect("concurrency semaphore is never closed")
    }

    /// Seeds the cached access token + expiry; used right after the sign-in code
    /// exchange, whose fresh access token would otherwise be discarded and immediately
    /// re-refreshed on the first sync.
    pub fn seed_access_token(&self, access_token: String, expires_at: OffsetDateTime) {
        let mut state = self
            .credential
            .state
            .lock()
            .expect("token state mutex poisoned");
        state.access_token = access_token;
        state.expires_at = expires_at;
        // A freshly signed-in account must not inherit a cool-down from the credential it
        // replaced.
        state.last_failure = None;
    }

    /// The cached access token, when it is still comfortably valid at `now`. Never holds the
    /// lock across an `await`.
    fn cached(&self, now: OffsetDateTime) -> Option<String> {
        let state = self
            .credential
            .state
            .lock()
            .expect("token state mutex poisoned");
        (!state.access_token.is_empty() && now + REFRESH_SKEW < state.expires_at)
            .then(|| state.access_token.clone())
    }

    /// The remembered outcome of a recent failed refresh, when it is still inside its
    /// cool-down: `(reach, message, seconds still to wait)`.
    ///
    /// This is the counterpart of [`GraphTokenSource::cached`] for the failure path, and the
    /// reason it exists is that it was missing. Serializing the refresh made concurrent
    /// callers share a *success*; on a failure each waiter in turn acquired the lock, found
    /// no cached token, and posted its own refresh presenting the **same** refresh token. One
    /// failure therefore became one request per waiting provider, and if the first request
    /// did reach the server, every one after it was a replay of a spent token, which is what
    /// a ratcheting server revokes the grant over.
    fn recent_failure(&self, now: OffsetDateTime) -> Option<(FailureKind, String, Duration)> {
        let state = self
            .credential
            .state
            .lock()
            .expect("token state mutex poisoned");
        let failure = state.last_failure.as_ref()?;
        let until = failure.at + failure.kind.cooldown();
        (now < until).then(|| (failure.kind, failure.message.clone(), until - now))
    }

    /// Remembers a failed refresh so the callers behind it take this outcome rather than
    /// re-presenting the same refresh token.
    fn note_failure(&self, at: OffsetDateTime, kind: FailureKind, message: String) {
        let mut state = self
            .credential
            .state
            .lock()
            .expect("token state mutex poisoned");
        state.last_failure = Some(RefreshFailure { at, kind, message });
    }

    /// Test-only: how the last refresh failed, and how long it suppresses further attempts.
    #[cfg(test)]
    fn last_failure(&self) -> Option<(FailureKind, i64)> {
        let state = self
            .credential
            .state
            .lock()
            .expect("token state mutex poisoned");
        state
            .last_failure
            .as_ref()
            .map(|failure| (failure.kind, failure.kind.cooldown().whole_seconds()))
    }

    /// Test-only: backdates the remembered failure, so a cool-down can be driven past its
    /// end without a test sleeping through it.
    #[cfg(test)]
    fn backdate_failure(&self, by: Duration) {
        let mut state = self
            .credential
            .state
            .lock()
            .expect("token state mutex poisoned");
        if let Some(failure) = state.last_failure.as_mut() {
            failure.at -= by;
        }
    }

    /// Returns a valid access token, refreshing (and caching) it when the current one is
    /// missing or within `REFRESH_SKEW` of expiry. A refresh that rotates the refresh
    /// token updates the cache and notifies the [`TokenSink`].
    ///
    /// **Exactly one refresh runs at a time per account**, and every other caller waits for it
    /// and takes its result; its *failure* as readily as its success. That is not an
    /// optimisation: see the `refreshing` field: a second refresh with the same refresh token
    /// is a *replay*, and a ratcheting authorization server answers it by revoking the grant.
    /// Sharing only the success is not enough, because a failure leaves nothing cached and each
    /// waiter would go on to present that same token itself.
    ///
    /// # Errors
    ///
    /// Returns [`AccountError::SigninRejected`] if the refresh token is revoked/expired
    /// (re-authenticate), or [`AccountError::Graph`] on a transient refresh failure.
    pub async fn access_token(&self) -> Result<String, AccountError> {
        // Fast path: a cached token still comfortably valid, taken without serializing.
        if let Some(token) = self.cached(OffsetDateTime::now_utc()) {
            return Ok(token);
        }
        // Only one refresh at a time. Whoever waited here may find the winner already cached a
        // fresh token, in which case there is nothing left to do; re-check *after* acquiring,
        // or every waiter would go on to post the replay this lock exists to prevent.
        let _refreshing = self.credential.refreshing.lock().await;
        let now = OffsetDateTime::now_utc();
        if let Some(token) = self.cached(now) {
            log::debug!("graph: another caller refreshed while we waited; using its token");
            return Ok(token);
        }
        // …and if the refresh we queued behind *failed*, take its failure. Posting our own
        // would present the same refresh token again, which is a replay whenever that request
        // reached the server. See `Self::recent_failure`.
        if let Some((kind, message, remaining)) = self.recent_failure(now) {
            log::debug!(
                "oauth: {} [{}]: a recent refresh already failed {}, so this caller takes that \
                 outcome instead of re-presenting the same refresh token; the next attempt is in \
                 {}s: {message}",
                self.provider,
                crate::account_log_handle(self.account.as_str()),
                kind.describe(),
                remaining.whole_seconds(),
            );
            return Err(kind.error(&message));
        }
        let (refresh_token, had_token, attempts) = {
            let mut state = self
                .credential
                .state
                .lock()
                .expect("token state mutex poisoned");
            state.attempts += 1;
            (
                state.refresh_token.clone(),
                !state.access_token.is_empty(),
                state.attempts,
            )
        };
        // INFO, not DEBUG. Token refresh is the single most consequential thing this app does
        // that a user can neither see nor trigger, and it was invisible in a support log at the
        // shipping level: a production account died of a refresh defect and the whole log
        // contained no line about a refresh ever happening. Every field here is a fact about the
        // protocol, never an address or a token, so it is safe under docs/logging.md.
        let provider = self.provider;
        // Which account this is. Several accounts refresh concurrently on a real device, so
        // without a handle these lines interleave into an unreadable stream; three identical
        // rotation lines read the same whether that was three accounts once or one account three
        // times, and those are a healthy launch and a refresh loop. Never the account id: see
        // `crate::log_handle`.
        let account_handle = crate::account_log_handle(self.account.as_str());
        // The third arm is not cosmetic. A *failed* refresh leaves `access_token` empty, so
        // every later attempt used to report "first use in this process", which reads as a
        // fresh, healthy launch. On a real log that turned one account retrying into what
        // looked like several token sources for one account, and sent an investigation after a
        // duplicate-source bug that does not exist.
        log::info!(
            "oauth: {provider} [{account_handle}]: refreshing the access token ({})",
            if had_token {
                "the cached one is within the expiry skew".to_owned()
            } else if attempts == 1 {
                "none cached; first use in this process".to_owned()
            } else {
                format!(
                    "still none cached; {} earlier attempt(s) in this process failed",
                    attempts - 1
                )
            },
        );
        let started = std::time::Instant::now();
        let tokens = match self.oauth.refresh(&refresh_token, now).await {
            Ok(tokens) => tokens,
            // Both refusals that never recover, and the line support reads first. A dead grant
            // (`invalid_grant`) and an under-scoped one (`invalid_scope`) mean different things
            // about the person (signed out versus still signed in) but for a token source they
            // are the same fact: this refresh token will not mint a token again, and only consent
            // changes that. Retrying either is what a caller does when it mistakes them for an
            // outage, and it never ends.
            Err(err) if err.refusal().needs_reauth() => {
                log::warn!(
                    "oauth: {provider} [{account_handle}]: the server REFUSED the stored refresh token \
                     after {}ms: the sign-in cannot be renewed and only a fresh one \
                     helps, not asking again for {}m: {err}",
                    started.elapsed().as_millis(),
                    DEAD_GRANT_COOLDOWN.whole_minutes(),
                );
                // Remembered like any other failure. Nothing about a dead grant changes by
                // asking again, and every provider on the account is queued behind this one.
                self.note_failure(now, FailureKind::DeadGrant, err.to_string());
                return Err(AccountError::SigninRejected(err.to_string()));
            }
            Err(err) => {
                // Two very different failures used to share this arm and this sentence. The
                // question that separates them is whether the server could have *processed*
                // the request; because if it did, it consumed the refresh token we presented
                // and the replacement was in the answer we never got. Re-presenting the old
                // one is then a replay, and a ratcheting server revokes the grant over it.
                let reach = err.reach();
                match reach {
                    TokenRequestReach::NotSent => log::warn!(
                        "oauth: {provider} [{account_handle}]: the token refresh could not leave this \
                         device after {}ms, so no server saw the refresh token and it is still \
                         good, not retrying for {}s: {err}",
                        started.elapsed().as_millis(),
                        NOT_SENT_COOLDOWN.whole_seconds(),
                    ),
                    // The line support should read first after a mysteriously dead account.
                    TokenRequestReach::MaybeProcessed => log::warn!(
                        "oauth: {provider} [{account_handle}]: the token refresh MAY have reached the \
                         server after {}ms; if it did, the refresh token we presented is spent \
                         and its replacement was lost with the response. Holding off {}s rather \
                         than re-presenting it, because a replay is what revokes a grant: {err}",
                        started.elapsed().as_millis(),
                        MAYBE_PROCESSED_COOLDOWN.whole_seconds(),
                    ),
                }
                self.note_failure(now, FailureKind::Unanswered(reach), err.to_string());
                return Err(AccountError::Graph(format!("token refresh: {err}")));
            }
        };
        let access = tokens.access_token.expose().to_owned();
        let rotated = tokens
            .refresh_token
            .as_ref()
            .map(|secret| secret.expose().to_owned());
        {
            let mut state = self
                .credential
                .state
                .lock()
                .expect("token state mutex poisoned");
            state.access_token.clone_from(&access);
            state.expires_at = tokens.expires_at;
            state.last_failure = None;
            if let Some(new_refresh) = &rotated {
                state.refresh_token.clone_from(new_refresh);
            }
        }
        // Persist a rotated refresh token so the next launch uses the current one. Reported
        // even with no sink wired, because "the server rotated and nobody stored it" is the
        // single most consequential thing that can happen here and it used to be silent: the
        // account works for the rest of the session on the token just minted, then fails at the
        // next launch with a `invalid_grant` that names nothing.
        log::info!(
            "oauth: {provider} [{account_handle}]: refreshed in {}ms; the access token is valid for {} \
             more minute(s), and the server {}",
            started.elapsed().as_millis(),
            (tokens.expires_at - OffsetDateTime::now_utc()).whole_minutes(),
            if rotated.is_some() {
                "ROTATED the refresh token"
            } else {
                "kept the same refresh token"
            },
        );
        match (self.sink.as_ref(), rotated.as_ref()) {
            (Some(sink), Some(new_refresh)) => {
                log::info!(
                    "oauth: [{account_handle}] the server rotated this account's refresh token; \
                     handing it to the host to re-persist",
                );
                sink.refresh_token_rotated(&self.account, new_refresh).await;
            }
            (None, Some(_)) => log::warn!(
                "oauth: [{account_handle}] the server rotated this account's refresh token but NO token \
                 sink is wired, so the new token is held in memory only: this account will fail \
                 to authenticate after the next restart",
            ),
            (_, None) => {}
        }
        Ok(access)
    }
}

/// Test-only helpers shared by this module's tests and the sibling provider tests: the mock
/// token endpoints (including the **ratcheting** one that reproduces a replay-detecting server)
/// and a [`GraphTokenSource`] pointed at them. Its own file, because the mock servers grew past
/// the point where they belonged in the middle of the production type.
#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests;
