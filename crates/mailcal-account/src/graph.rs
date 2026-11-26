//! Building the Microsoft Graph mail providers a Microsoft account syncs through.
//!
//! Every provider of an account asks its one [`GraphTokenSource`] for the token on each request,
//! through the engine's credential source: the source refreshes a token near its expiry, and
//! replaces one Graph refused, single-flight, so one refresh serves every folder. A rotated
//! refresh token is reported to the host via [`TokenSink`] to be re-persisted in the OS keystore.
//!
//! The folders [`connect_graph_mail_providers`] binds share one [`GraphClient`], and with it one
//! connection pool; a folder opened on demand ([`connect_graph_folder`]), the calendar and the
//! contact sources each connect a client of their own. Each folder's [`ReconnectingGraphProvider`]
//! forwards to a [`GraphProvider`] on its client and replaces the client after a request fails on
//! a dead connection.

use std::sync::Arc;

use engine_api::{RetryConfig, UserAgent};
use engine_core::{
    ids::{AccountId, MailboxId, SharedAccountId},
    mail::Mailbox,
    sync::SyncUpdate,
    time::CalendarDate,
};
use engine_provider::{
    Capabilities, CredentialSource, IdentityControls, Provider, ProviderError, ProviderResult,
    ReportControls, ReportEvidence, ReportVerdicts, SharedAccounts,
};
use engine_tls::TlsClientConfig;
use provider_graph::{GraphClient, GraphError, GraphProvider, MailboxPrincipal};
use time::Date;

mod affiliation;
mod calendar;
mod contacts;
mod mail_provider;
pub(crate) mod token_source;

pub use affiliation::{google_affiliation, graph_affiliation};
pub use calendar::connect_graph_calendar_provider;
pub use contacts::connect_graph_contact_providers;
#[cfg(test)]
pub(crate) use token_source::test_support;
pub use token_source::{CredentialOrigin, GraphTokenSource, TokenSink};

use crate::{
    AccountError,
    log_handle::account_log_handle,
    pass_syncs,
    shared_client::{Delegate, SharedClient},
    tls::tls_with,
};

/// A [`Provider`] bound to one Graph mail folder that forwards to a [`GraphProvider`] on the
/// account's shared client, and replaces that client once after a request fails on a dead
/// connection. Internal; the account layer hands callers `Box<dyn Provider>` from
/// [`connect_graph_mail_providers`] / [`connect_graph_folder`], never the concrete type.
#[derive(Debug)]
pub(crate) struct ReconnectingGraphProvider {
    folder: MailboxId,
    capabilities: Capabilities,
    delegate: Delegate<GraphClient, GraphProvider>,
}

impl ReconnectingGraphProvider {
    /// Binds a provider to `folder` on `client` (windowed to `since`).
    ///
    /// # Errors
    ///
    /// When the client cannot be built.
    pub(crate) fn new(
        folder: MailboxId,
        client: Arc<SharedClient<GraphClient>>,
        since: Option<Date>,
    ) -> ProviderResult<Self> {
        let bound = folder.clone();
        let delegate = Delegate::new(client, move |client| {
            let graph = GraphProvider::new(client, bound.clone())
                .with_keyword_names(crate::drafted_reply_keyword_names());
            match since.and_then(calendar_date) {
                Some(date) => graph.with_since(date),
                None => graph,
            }
        })?;
        Ok(Self {
            folder,
            capabilities: Capabilities::none()
                .with_mail()
                .with_mail_writes()
                // Forwarded by this wrapper's `MailboxWrites`.
                .with_mailbox_writes()
                // Forwarded by this wrapper's `Provider`. A flag omitted here is a
                // flag the account does not have however loudly the delegate advertises it,
                // so advertising and forwarding have to move together.
                .with_mail_report(ReportControls {
                    verdicts: ReportVerdicts::all(),
                    evidence: ReportEvidence::Acknowledged,
                })
                // Forwarded by this wrapper's `Provider`: `put_draft` and `delete_draft`.
                .with_mail_drafts()
                .with_submission()
                // The whole `Draft` is forwarded to the delegate untouched, so whatever it can
                // send, this wrapper can. Graph submits assembled RFC 5322 bytes and therefore
                // owns the `method=` parameter that makes an iTIP object a *scheduling*
                // message rather than a calendar file (RFC 6047 §2.4).
                .with_scheduling_submission()
                // Every delegate is built with the drafted-reply category's names, which is what
                // makes Graph keep a Sent copy's keyword. Graph files the copy after its send
                // answers, so the keyword is applied afterwards by an edit the outbox queues
                // (deferred), which this wrapper's `edit_mail` forwards.
                .with_sent_copy_keywords()
                .with_sent_copy_keywords_deferred()
                // The same names keep a stored draft's keyword, forwarded with `put_draft`.
                .with_draft_keywords()
                // Forwarded below, and **read-only**: a Graph mailbox's display name is a
                // directory attribute a tenant administrator owns, so claiming `Writable`
                // here would put an editor in front of an edit that cannot land.
                .with_sender_identities(IdentityControls::ReadOnly)
                // Forwarded by this wrapper's `Provider`: a fact about the sign-in, whichever
                // mailbox this provider is bound to.
                .with_shared_accounts(SharedAccounts::ByAddress),
            delegate,
        })
    }
}

/// The client every Graph provider of an account shares: requests rooted at `principal`'s
/// mailbox, each asking `tokens` for the token.
pub(crate) fn account_client(
    tokens: Arc<GraphTokenSource>,
    principal: MailboxPrincipal,
    tls: TlsClientConfig,
) -> Arc<SharedClient<GraphClient>> {
    client_connecting(tokens, principal, tls, GraphClient::connect_with_source)
}

/// [`account_client`], each client connected by `connect`: Graph's root, or a test's server.
pub(crate) fn client_connecting(
    tokens: Arc<GraphTokenSource>,
    principal: MailboxPrincipal,
    tls: TlsClientConfig,
    connect: impl Fn(
        Arc<dyn CredentialSource>,
        &TlsClientConfig,
        &RetryConfig,
        &UserAgent,
    ) -> Result<GraphClient, GraphError>
    + Send
    + Sync
    + 'static,
) -> Arc<SharedClient<GraphClient>> {
    Arc::new(SharedClient::new(connector(
        tokens, principal, tls, connect,
    )))
}

/// What builds one Graph client of the account: `connect` over `tokens`, rooted at
/// `principal`'s mailbox.
pub(crate) fn connector(
    tokens: Arc<GraphTokenSource>,
    principal: MailboxPrincipal,
    tls: TlsClientConfig,
    connect: impl Fn(
        Arc<dyn CredentialSource>,
        &TlsClientConfig,
        &RetryConfig,
        &UserAgent,
    ) -> Result<GraphClient, GraphError>
    + Send
    + Sync
    + 'static,
) -> impl Fn() -> ProviderResult<GraphClient> + Send + Sync + 'static {
    move || {
        connect(
            Arc::clone(&tokens) as Arc<dyn CredentialSource>,
            &tls,
            &tokens.retry(),
            &crate::user_agent(),
        )
        .map(|client| client.with_principal(principal.clone()))
        .map_err(ProviderError::from)
    }
}

fn calendar_date(date: Date) -> Option<CalendarDate> {
    CalendarDate::new(date.year(), u8::from(date.month()), date.day()).ok()
}

/// Connects the Graph mail providers a Microsoft account syncs: one per folder the account
/// lists that [`pass_syncs`], all on one client. Enumerates the account's folders once, then
/// binds a `ReconnectingGraphProvider` to each folder's id: the Graph parallel of
/// [`connect_mail_providers`](crate::connect_mail_providers), and bound in full for the same
/// reason, since an account pass syncs what is bound and nothing else. The `tokens` source
/// carries the account's credentials and id. With `shared`, the folders are those of the mailbox
/// shared with the account that it names.
///
/// # Errors
///
/// Returns [`AccountError`] if the initial token refresh or the folder-list sync fails, or
/// `shared` is no handle this adapter produces.
pub async fn connect_graph_mail_providers(
    account_id: &AccountId,
    tokens: Arc<GraphTokenSource>,
    since: Option<Date>,
    shared: Option<&SharedAccountId>,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    let tls = tls_with(&[])?;
    // A dead grant fails here as a sign-in to repeat, rather than as a folder list that failed.
    tokens.access_token().await?;
    let client = account_client(tokens, principal(shared)?, tls);
    let folders = list_folders(&client, account_id).await?;
    folders
        .into_iter()
        .filter(pass_syncs)
        .map(|mailbox| {
            ReconnectingGraphProvider::new(mailbox.id, Arc::clone(&client), since)
                .map(|provider| Box::new(provider) as Box<dyn Provider>)
                .map_err(|err| AccountError::Graph(err.to_string()))
        })
        .collect()
}

/// Builds a Graph provider bound to one folder of a Microsoft account (one listed after the
/// account connected, or opened before any pass synced it), asking the account's `tokens`. Sync;
/// nothing is sent before the first call. The Graph parallel of
/// [`connect_imap_mailbox`](crate::connect_imap_mailbox). `shared` names the shared mailbox the
/// folder belongs to.
///
/// # Errors
///
/// Returns [`AccountError::Mailbox`] if `mailbox_key` is not a valid folder id, and
/// [`AccountError::Graph`] if `shared` is no handle this adapter produces or the client cannot be
/// built.
pub fn connect_graph_folder(
    tokens: Arc<GraphTokenSource>,
    mailbox_key: &str,
    since: Option<Date>,
    shared: Option<&SharedAccountId>,
) -> Result<Box<dyn Provider>, AccountError> {
    let folder =
        MailboxId::try_from(mailbox_key).map_err(|err| AccountError::Mailbox(err.to_string()))?;
    let client = account_client(tokens, principal(shared)?, tls_with(&[])?);
    let provider = ReconnectingGraphProvider::new(folder, client, since)
        .map_err(|err| AccountError::Graph(err.to_string()))?;
    Ok(Box::new(provider))
}

/// The signed-in person's own mailbox, or the shared one `shared` names.
fn principal(shared: Option<&SharedAccountId>) -> Result<MailboxPrincipal, AccountError> {
    shared.map_or(Ok(MailboxPrincipal::Me), |handle| {
        MailboxPrincipal::shared(handle).map_err(|err| AccountError::Graph(err.to_string()))
    })
}

/// Enumerates the account's mail folders (a full snapshot) on the account's client. The bound
/// folder is irrelevant to the folder-list call, so any well-known alias serves to construct the
/// probe.
async fn list_folders(
    client: &SharedClient<GraphClient>,
    account_id: &AccountId,
) -> Result<Vec<Mailbox>, AccountError> {
    let (_, client) = client
        .current()
        .map_err(|err| AccountError::Graph(err.to_string()))?;
    let inbox =
        MailboxId::try_from("inbox").map_err(|err| AccountError::Mailbox(err.to_string()))?;
    // Both lines carry the account's handle. One device holds several Microsoft accounts, this
    // runs once per account, and two counts minutes apart read as one mailbox's list shrinking
    // unless each says whose it was (`docs/logging.md`). The handle, never the id: an account
    // id is an address and a host.
    let handle = account_log_handle(account_id.as_str());
    log::debug!("graph[{handle}]: fetching mail folder list");
    let listing = GraphProvider::new(client, inbox)
        .sync_mailboxes(account_id, None)
        .await
        .map_err(|err| AccountError::MailboxList(err.to_string()))?;
    let folders = match listing.update {
        SyncUpdate::Snapshot { objects, .. } => objects,
        SyncUpdate::Delta { changed, .. } => changed,
    };
    log::debug!(
        "graph[{handle}]: folder list returned {} folder(s)",
        folders.len()
    );
    Ok(folders)
}

#[cfg(test)]
mod tests {
    use engine_core::sync::SyncScope;

    use super::{
        token_source::test_support::{
            assert_drafts_reach_the_delegate, dead_grant_token_endpoint, mock_token_endpoint,
            source_at,
        },
        *,
    };

    /// A draft saved in an Outlook account reached the trait's default and failed with "provider
    /// does not support mail drafts", although the Graph adapter implements both verbs: this
    /// wrapper reports a capability set of its own and forwards each verb by hand, and drafts
    /// were in neither.
    #[tokio::test]
    async fn drafts_are_advertised_and_forwarded_to_graph() {
        let (endpoint, _hits) = dead_grant_token_endpoint();
        let provider =
            connect_graph_folder(source_at(endpoint, None), "drafts", None, None).unwrap();
        assert_drafts_reach_the_delegate(provider.as_ref()).await;
    }

    #[test]
    fn connect_graph_folder_binds_a_mail_read_write_send_provider() {
        let (endpoint, _hits) = mock_token_endpoint(vec![]);
        let source = source_at(endpoint, None);
        let account = AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap();
        let provider = connect_graph_folder(source, "custom-folder-id", None, None).unwrap();
        // Mail read + writes (mark-read/flag, move, delete) + submission (sendMail is
        // account-level, so any folder-bound provider advertises it), no calendar; all
        // reported before any delegate is built (no network yet), so a host can gate mail
        // actions and the composer's send up front. The per-folder scope names the bound folder.
        assert!(provider.connection_info().capabilities.mail());
        assert!(provider.connection_info().capabilities.mail_writes());
        assert!(provider.connection_info().capabilities.submission());
        assert!(provider.connection_info().capabilities.sent_copy_keywords());
        assert!(
            provider
                .connection_info()
                .capabilities
                .sent_copy_keywords_deferred()
        );
        assert!(provider.connection_info().capabilities.draft_keywords());
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
