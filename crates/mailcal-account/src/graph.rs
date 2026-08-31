//! Building the Microsoft Graph mail providers a Microsoft account syncs through, with
//! automatic OAuth token refresh.
//!
//! The engine's [`GraphProvider`] takes a **static** bearer token, but Graph access
//! tokens live ~1 hour; far shorter than an app session. So this module wraps it in a
//! [`RefreshingGraphProvider`] that mints a fresh access token (refreshing from the
//! stored refresh token when the cached one is stale) **before each network call**, then
//! delegates to a freshly built `GraphProvider`. Graph is stateless HTTP: no socket to
//! keep alive: so rebuilding the client per call is cheap. A shared [`GraphTokenSource`]
//! backs every folder's provider (and on-demand opens) so one refresh serves them all,
//! and a rotated refresh token is reported to the host via [`TokenSink`] to be
//! re-persisted in the OS keystore.

use std::sync::{Arc, Mutex};

use engine_core::{
    error::FailureClass,
    ids::{AccountId, MailboxId},
    mail::{Mailbox, MailboxRole},
    sync::SyncUpdate,
    time::CalendarDate,
};
use engine_provider::{
    Capabilities, Provider, ProviderError, ProviderResult, ReportControls, ReportEvidence,
    ReportVerdicts,
};
use engine_tls::TlsClientConfig;
use provider_graph::{GraphClient, GraphProvider, MailboxPrincipal};
use time::Date;

mod calendar;
mod mail_provider;
mod token_source;

pub use calendar::connect_graph_calendar_providers;
pub use token_source::{CredentialOrigin, GraphTokenSource, TokenSink};

use crate::{AccountError, throttle::account_retry, tls::account_tls};

/// The folder roles a Microsoft account eagerly binds a provider to at startup; the
/// same set as IMAP plus the Inbox (Graph resolves the Inbox as a role, whereas IMAP
/// connects the literal `INBOX` separately). Any other folder (a custom folder, role
/// `None`) syncs **on demand** via [`connect_graph_folder`].
const GRAPH_SYNCED_ROLES: &[MailboxRole] = &[
    MailboxRole::Inbox,
    MailboxRole::Sent,
    MailboxRole::Drafts,
    MailboxRole::Trash,
    MailboxRole::Archive,
    MailboxRole::Junk,
];

/// A [`Provider`] bound to one Graph mail folder that refreshes its access token before
/// every network call and delegates to a freshly built [`GraphProvider`]. Internal; the
/// account layer hands callers `Box<dyn Provider>` from [`connect_graph_mail_providers`]
/// / [`connect_graph_folder`], never the concrete type.
#[derive(Debug)]
pub(crate) struct RefreshingGraphProvider {
    folder: MailboxId,
    tokens: Arc<GraphTokenSource>,
    capabilities: Capabilities,
    /// The sync-depth cutoff, applied to the built delegate so the initial snapshot is
    /// windowed to recent mail (`None` syncs the whole folder).
    since: Option<Date>,
    /// The account's shared TLS policy, cloned into each rebuilt Graph client.
    tls: TlsClientConfig,
    /// The built delegate, cached by the access token it was built with, so its reqwest
    /// **connection pool is reused** across requests. Only rebuilt when the token refreshes
    /// (~hourly); never per request, which would open a fresh TLS connection every time
    /// (the connection storm that hammered Graph before this cache).
    cached: Mutex<Option<(String, Arc<GraphProvider>)>>,
}

impl RefreshingGraphProvider {
    /// Binds a refreshing provider to `folder` (windowed to `since`), sharing `tokens`
    /// with the account's other folders.
    #[must_use]
    pub(crate) fn new(
        folder: MailboxId,
        tokens: Arc<GraphTokenSource>,
        since: Option<Date>,
        tls: TlsClientConfig,
    ) -> Self {
        Self {
            folder,
            tokens,
            capabilities: Capabilities::none()
                .with_mail()
                .with_mail_writes()
                // Forwarded by this wrapper's `Provider`. A flag omitted here is a
                // flag the account does not have however loudly the delegate advertises it,
                // so advertising and forwarding have to move together.
                .with_mail_report(ReportControls {
                    verdicts: ReportVerdicts::all(),
                    evidence: ReportEvidence::Acknowledged,
                })
                .with_submission()
                // The whole `Draft` is forwarded to the delegate untouched, so whatever it can
                // send, this wrapper can. Graph submits assembled RFC 5322 bytes and therefore
                // owns the `method=` parameter that makes an iTIP object a *scheduling*
                // message rather than a calendar file (RFC 6047 §2.4).
                .with_scheduling_submission(),
            since,
            tls,
            cached: Mutex::new(None),
        }
    }

    /// Returns the delegate `GraphProvider`, reusing the cached one while the access token
    /// is unchanged and rebuilding it only when a refresh produced a new token: so the
    /// underlying HTTP client (and its warm connection pool) is reused across requests.
    async fn delegate(&self) -> ProviderResult<Arc<GraphProvider>> {
        let token = self.tokens.access_token().await.map_err(|err| match err {
            AccountError::SigninRejected(detail) => ProviderError::authentication(detail),
            other => ProviderError::retryable(other.to_string()),
        })?;
        {
            let cache = self.cached.lock().expect("graph delegate mutex poisoned");
            if let Some((cached_token, provider)) = cache.as_ref()
                && *cached_token == token
            {
                return Ok(Arc::clone(provider));
            }
        }
        let client = GraphClient::for_mailbox(
            token.clone(),
            MailboxPrincipal::Me,
            &self.tls,
            &account_retry(),
        )
        .map_err(ProviderError::from)?;
        let mut graph = GraphProvider::new(client, self.folder.clone());
        if let Some(date) = self.since.and_then(calendar_date) {
            graph = graph.with_since(date);
        }
        let provider = Arc::new(graph);
        *self.cached.lock().expect("graph delegate mutex poisoned") =
            Some((token, Arc::clone(&provider)));
        Ok(provider)
    }

    /// Drops the cached delegate so the next [`delegate`](Self::delegate) rebuilds a fresh
    /// [`GraphClient`], and with it a fresh reqwest connection pool. Used after a retryable
    /// transport failure, whose keep-alive socket (e.g. one the OS killed while the machine
    /// slept) is presumed dead; the token is unchanged, so this only rebuilds the client.
    fn invalidate_delegate(&self) {
        *self.cached.lock().expect("graph delegate mutex poisoned") = None;
    }
}

/// Whether a failed Graph call warrants dropping the cached delegate and rebuilding it: a
/// retryable **transport** error (a stale keep-alive socket after sleep, a broken pipe) that
/// a fresh client and connection pool may clear. Distinct from a 429, which is waited out one
/// layer down on the same connection, and from an auth failure (never retried). Tried at
/// most once per call. For the reads that is trivially safe (idempotent GETs), for the wrapped
/// mutation ([`edit_mail`](RefreshingGraphProvider::edit_mail)) it is safe because a transport
/// error on Graph's stateless HTTP means the request died at the socket (a reset keep-alive)
/// before the server applied it, so re-dialing and re-issuing once cannot double-apply, and
/// the edit is outbox-mediated regardless.
///
/// [`submit_email`](RefreshingGraphProvider::submit_email) is the exception: it uses this only
/// to **drop** a dead delegate, never to re-issue, because a send is not idempotent (a resent
/// `sendMail` double-delivers) and a `Retryable` transport error cannot tell a lost-before-send
/// from a lost-after-send. See that method for why the outbox (not this wrapper) owns the
/// retry.
fn should_reconnect(err: &ProviderError) -> bool {
    err.class() == FailureClass::Retryable
}

fn calendar_date(date: Date) -> Option<CalendarDate> {
    CalendarDate::new(date.year(), u8::from(date.month()), date.day()).ok()
}

/// Connects the Graph mail providers a Microsoft account syncs: one per eagerly bound
/// role folder (`GRAPH_SYNCED_ROLES`). Enumerates the account's folders once (a fresh
/// token), then binds a shared-token `RefreshingGraphProvider` to each role folder's
/// real id: the Graph parallel of [`connect_mail_providers`](crate::connect_mail_providers).
/// The `tokens` source carries the account's credentials and id.
///
/// # Errors
///
/// Returns [`AccountError`] if the initial token refresh or the folder-list sync fails.
pub async fn connect_graph_mail_providers(
    account_id: &AccountId,
    tokens: Arc<GraphTokenSource>,
    since: Option<Date>,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    let tls = account_tls()?;
    let folders = list_folders(&tokens, account_id, &tls).await?;
    let providers = folders
        .into_iter()
        .filter(|mailbox| {
            mailbox
                .role
                .as_ref()
                .is_some_and(|role| GRAPH_SYNCED_ROLES.contains(role))
        })
        .map(|mailbox| {
            Box::new(RefreshingGraphProvider::new(
                mailbox.id,
                Arc::clone(&tokens),
                since,
                tls.clone(),
            )) as Box<dyn Provider>
        })
        .collect();
    Ok(providers)
}

/// Builds an on-demand Graph provider bound to one folder of a Microsoft account (a
/// custom folder the eager bind skipped), sharing the account's `tokens`. Sync; the
/// token is fetched lazily on the first call. The Graph parallel of
/// [`connect_imap_mailbox`](crate::connect_imap_mailbox).
///
/// # Errors
///
/// Returns [`AccountError::Mailbox`] if `mailbox_key` is not a valid folder id.
pub fn connect_graph_folder(
    tokens: Arc<GraphTokenSource>,
    mailbox_key: &str,
    since: Option<Date>,
) -> Result<Box<dyn Provider>, AccountError> {
    let folder =
        MailboxId::try_from(mailbox_key).map_err(|err| AccountError::Mailbox(err.to_string()))?;
    let tls = account_tls()?;
    Ok(Box::new(RefreshingGraphProvider::new(
        folder, tokens, since, tls,
    )))
}

/// Enumerates the account's mail folders (a full snapshot), using a one-off client with a
/// fresh access token. The bound folder is irrelevant to the folder-list call, so any
/// well-known alias serves to construct the probe.
async fn list_folders(
    tokens: &Arc<GraphTokenSource>,
    account_id: &AccountId,
    tls: &TlsClientConfig,
) -> Result<Vec<Mailbox>, AccountError> {
    let token = tokens.access_token().await?;
    let client = GraphClient::for_mailbox(token, MailboxPrincipal::Me, tls, &account_retry())
        .map_err(|err| AccountError::Graph(err.to_string()))?;
    let inbox =
        MailboxId::try_from("inbox").map_err(|err| AccountError::Mailbox(err.to_string()))?;
    log::debug!("graph: fetching mail folder list");
    let listing = GraphProvider::new(client, inbox)
        .sync_mailboxes(account_id, None)
        .await
        .map_err(|err| AccountError::MailboxList(err.to_string()))?;
    let folders = match listing.update {
        SyncUpdate::Snapshot { objects, .. } => objects,
        SyncUpdate::Delta { changed, .. } => changed,
    };
    log::debug!("graph: folder list returned {} folder(s)", folders.len());
    Ok(folders)
}

#[cfg(test)]
mod tests {
    use engine_core::sync::SyncScope;

    use super::{
        token_source::test_support::{mock_token_endpoint, source_at},
        *,
    };

    #[test]
    fn a_retryable_transport_error_triggers_a_reconnect_but_a_rate_limit_or_auth_does_not() {
        // A retryable transport failure (a stale keep-alive socket after sleep) drops the
        // cached delegate and rebuilds a fresh client + connection pool…
        assert!(should_reconnect(&ProviderError::retryable("broken pipe")));
        // …while a 429 is waited out on the *same* connection one layer down, not by a
        // reconnect, and an auth failure is never a transport reconnect (the host re-auths).
        assert!(!should_reconnect(&ProviderError::new(
            FailureClass::RateLimited,
            "429",
        )));
        assert!(!should_reconnect(&ProviderError::authentication("401")));
    }

    #[test]
    fn connect_graph_folder_binds_a_mail_read_write_send_provider() {
        let (endpoint, _hits) = mock_token_endpoint(vec![]);
        let source = source_at(endpoint, None);
        let account = AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap();
        let provider = connect_graph_folder(source, "custom-folder-id", None).unwrap();
        // Mail read + writes (mark-read/flag, move, delete) + submission (sendMail is
        // account-level, so any folder-bound provider advertises it), no calendar; all
        // reported before any delegate is built (no network yet), so a host can gate mail
        // actions and the composer's send up front. The per-folder scope names the bound folder.
        assert!(provider.connection_info().capabilities.mail());
        assert!(provider.connection_info().capabilities.mail_writes());
        assert!(provider.connection_info().capabilities.submission());
        // And specifically that it can send an *iMIP* message: a `Draft` carrying a
        // `text/calendar` part with `method=`. Sending ordinary mail is a different promise:
        // an account that can do the first and not the second cannot answer an invitation on
        // a calendar server that does no scheduling, and the card has to say so rather than
        // offering three buttons that go nowhere.
        assert!(
            provider
                .connection_info()
                .capabilities
                .scheduling_submission()
        );
        assert!(!provider.connection_info().capabilities.calendars());
        assert_eq!(
            provider.email_scope(&account),
            SyncScope::GraphFolder {
                account: account.clone(),
                folder: MailboxId::try_from("custom-folder-id").unwrap(),
            }
        );
    }
}
