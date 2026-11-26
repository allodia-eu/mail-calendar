//! Building the Google **calendar** provider a Google account syncs its agenda through: the
//! calendar parallel of the Gmail provider in the parent module, and the Google sibling of
//! [`crate::graph::connect_graph_calendar_provider`].
//!
//! One [`ReconnectingGoogleCalendarProvider`] serves every calendar of the account and forwards
//! to a [`GoogleCalendarProvider`] on a client that asks the account's token source for the token
//! on each request. The engine names the window each event fetch opens over
//! (`docs/calendar.md` §4).
//!
//! Two things are simpler than the Graph calendar: Google is **IANA-native** (event times carry
//! an IANA `timeZone`), so there is **no display-zone `Prefer` header** to thread through, and
//! Google requests the calendar scope at connect time alongside mail, so there is no
//! "connected before calendar support" re-consent case: a calendar failure is simply
//! non-fatal (mail stays up with an empty agenda).
//!
//! **Reads** (`sync_calendars`/`sync_events`) are idempotent, so they get the one-shot reconnect
//! loop the mail provider uses. **Writes** (`create`/`patch`/`delete`) are single-attempt (a
//! blind retry of a non-idempotent call could double-create) and a failed write surfaces to the
//! app to retry; one that met a dead connection still replaces the client.

use std::sync::Arc;

use async_trait::async_trait;
use engine_api::{CalendarSync, CalendarWrites};
use engine_core::{
    calendar::{Calendar, Event},
    ids::{AccountId, CalendarId},
    sync::{SyncScope, SyncState, SyncUpdate},
    time::Horizon,
};
use engine_provider::{
    CalendarListing, CalendarUserAddresses, ConnectionInfo, EventDeletion, EventDraft, EventEdit,
    EventRsvp, EventWriteReceipt, Provider, ProviderResult, ScopeSync,
};
use provider_google::{GoogleCalendarProvider, GoogleClient};

use super::account_client;
use crate::{
    AccountError, GraphTokenSource,
    shared_client::{Delegate, SharedClient},
    tls::tls_with,
};

/// A [`Provider`] for every calendar of a Google account that forwards to a
/// [`GoogleCalendarProvider`] and replaces its client once after a read fails on a dead
/// connection. Internal; the account layer hands callers `Box<dyn Provider>` from
/// [`connect_google_calendar_provider`].
#[derive(Debug)]
pub(crate) struct ReconnectingGoogleCalendarProvider {
    delegate: Delegate<GoogleClient, GoogleCalendarProvider>,
}

impl ReconnectingGoogleCalendarProvider {
    /// A calendar provider on `client`.
    ///
    /// # Errors
    ///
    /// When the client cannot be built.
    pub(crate) fn new(client: Arc<SharedClient<GoogleClient>>) -> ProviderResult<Self> {
        Ok(Self {
            delegate: Delegate::new(client, GoogleCalendarProvider::new)?,
        })
    }
}

/// The engine adapter's own answers, capabilities included: a server-enforced write guard
/// (Google rejects a stale `If-Match` with a `412`), what a series edit costs the occurrences
/// changed by hand, server-side scheduling (`sendUpdates`, so a host sends no iMIP answer of its
/// own), and the engine naming the window a snapshot opens over.
#[async_trait]
impl Provider for ReconnectingGoogleCalendarProvider {
    fn connection_info(&self) -> ConnectionInfo {
        self.delegate.last().connection_info()
    }
}

#[async_trait]
impl CalendarSync for ReconnectingGoogleCalendarProvider {
    fn calendar_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::GoogleCalendarList {
            account: account.clone(),
        }
    }

    fn event_scope(&self, account: &AccountId, calendar: &CalendarId) -> SyncScope {
        SyncScope::GoogleCalendar {
            account: account.clone(),
            calendar: calendar.clone(),
        }
    }

    async fn sync_calendars(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<CalendarListing> {
        self.delegate
            .retrying(|provider| async move { provider.sync_calendars(account, cursor).await })
            .await
    }

    async fn sync_events(
        &self,
        account: &AccountId,
        calendar: &CalendarId,
        cursor: Option<&SyncState>,
        window: Horizon,
    ) -> ProviderResult<ScopeSync<Event>> {
        self.delegate
            .retrying(|provider| async move {
                provider
                    .sync_events(account, calendar, cursor, window)
                    .await
            })
            .await
    }
}

impl engine_api::MailboxWrites for ReconnectingGoogleCalendarProvider {}

#[async_trait]
impl CalendarWrites for ReconnectingGoogleCalendarProvider {
    async fn create_event(
        &self,
        account: &AccountId,
        draft: &EventDraft,
    ) -> ProviderResult<EventWriteReceipt> {
        // A create is non-idempotent; a blind retry could double-create, so single attempt.
        self.delegate
            .once(|provider| async move { provider.create_event(account, draft).await })
            .await
    }

    async fn patch_event(
        &self,
        account: &AccountId,
        base: &Event,
        edit: &EventEdit,
    ) -> ProviderResult<EventWriteReceipt> {
        // `If-Match`-guarded, so a retry after a partial success risks a spurious `412`.
        self.delegate
            .once(|provider| async move { provider.patch_event(account, base, edit).await })
            .await
    }

    async fn delete_event(
        &self,
        account: &AccountId,
        base: Option<&Event>,
        deletion: &EventDeletion,
    ) -> ProviderResult<()> {
        self.delegate
            .once(|provider| async move { provider.delete_event(account, base, deletion).await })
            .await
    }

    async fn rsvp_event(
        &self,
        account: &AccountId,
        base: &Event,
        rsvp: &EventRsvp,
    ) -> ProviderResult<EventWriteReceipt> {
        // Guarded like a patch, so a single attempt.
        self.delegate
            .once(|provider| async move { provider.rsvp_event(account, base, rsvp).await })
            .await
    }

    async fn calendar_user_addresses(
        &self,
        account: &AccountId,
    ) -> ProviderResult<CalendarUserAddresses> {
        self.delegate
            .retrying(|provider| async move { provider.calendar_user_addresses(account).await })
            .await
    }
}

/// Connects the Google calendar provider a Google account syncs its agenda through: **one**
/// provider for every calendar of the account: the Google parallel of
/// [`connect_graph_calendar_provider`](crate::connect_graph_calendar_provider). Reads
/// `calendarList` once first, so a calendar the account cannot reach fails here rather than on
/// every pass.
///
/// The caller connects this only for its non-fatal effect: a calendar failure leaves mail up
/// with an empty agenda.
///
/// # Errors
///
/// Returns [`AccountError`] if the initial token refresh or the calendar-list read fails.
pub async fn connect_google_calendar_provider(
    account_id: &AccountId,
    tokens: Arc<GraphTokenSource>,
) -> Result<Box<dyn Provider>, AccountError> {
    let tls = tls_with(&[])?;
    // A dead grant fails here as a sign-in to repeat, rather than as a calendar list that failed.
    tokens.access_token().await?;
    let google = |err: engine_provider::ProviderError| AccountError::Google(err.to_string());
    let provider = ReconnectingGoogleCalendarProvider::new(Arc::new(account_client(tokens, tls)))
        .map_err(google)?;
    log::debug!("google: fetching calendar list");
    let listing = provider
        .delegate
        .last()
        .sync_calendars(account_id, None)
        .await
        .map_err(google)?;
    let calendars: Vec<Calendar> = match listing.sync.update {
        SyncUpdate::Snapshot { objects, .. } => objects,
        SyncUpdate::Delta { changed, .. } => changed,
    };
    log::debug!(
        "google: calendar list returned {} calendar(s)",
        calendars.len()
    );
    Ok(Box::new(provider))
}

#[cfg(test)]
mod tests {
    use engine_provider::WriteGuard;
    use mailcal_oauth::{GOOGLE_SCOPES, OAuthClient, OAuthProviderConfig};

    use super::{super::client_connecting, *};
    use crate::{
        graph::test_support::{
            an_invitation, assert_calendar_reaches_the_delegate, dead_grant_token_endpoint,
            mock_token_endpoint, source_at,
        },
        http_fake,
    };

    /// An answer to an invitation may have been applied before the connection died, so it is
    /// sent once; the dead client is still replaced for the next request.
    #[tokio::test]
    async fn a_write_on_a_dead_connection_is_sent_once_and_the_client_replaced() {
        let (endpoint, _) = mock_token_endpoint(vec![http_fake::minted(1)]);
        let (base, received) = http_fake::serve(vec![http_fake::Reply::HangUp]);
        let client = client_connecting(
            source_at(endpoint, None),
            tls_with(&[]).unwrap(),
            move |source, tls, retry, agent| {
                GoogleClient::with_base_and_source(source, &base, tls, retry, agent)
            },
        );
        let provider = ReconnectingGoogleCalendarProvider::new(Arc::new(client)).unwrap();
        let account = AccountId::try_from("alice@gmail.com@mail.google.com").unwrap();
        let (event, rsvp) = an_invitation();

        provider
            .rsvp_event(&account, &event, &rsvp)
            .await
            .unwrap_err();

        assert_eq!(received.lock().unwrap().len(), 1, "sent once");
        assert_eq!(provider.delegate.generation(), 1, "a new client");
    }

    /// Each request asks the account's source for the token, and one Google refused is replaced
    /// once and the request sent again, on the same client.
    #[tokio::test]
    async fn each_request_presents_the_current_token_and_a_refused_one_is_replaced() {
        let (endpoint, minted) =
            mock_token_endpoint(vec![http_fake::minted(1), http_fake::minted(2)]);
        let (base, received) = http_fake::serve(http_fake::refused_then(r#"{"items":[]}"#));
        let client = client_connecting(
            source_at(endpoint, None),
            tls_with(&[]).unwrap(),
            move |source, tls, retry, agent| {
                GoogleClient::with_base_and_source(source, &base, tls, retry, agent)
            },
        );
        let provider = ReconnectingGoogleCalendarProvider::new(Arc::new(client)).unwrap();
        let account = AccountId::try_from("alice@gmail.com@mail.google.com").unwrap();

        provider.sync_calendars(&account, None).await.unwrap();
        provider.sync_calendars(&account, None).await.unwrap();

        http_fake::assert_replaced_once(&received, &minted);
    }

    /// The Google sibling of the Graph wrapper's test: a calendar fetch and an answer to an
    /// invitation reach the adapter rather than the traits' unsupported defaults.
    #[tokio::test]
    async fn calendar_calls_are_forwarded_to_google() {
        let (endpoint, _hits) = dead_grant_token_endpoint();
        let provider = ReconnectingGoogleCalendarProvider::new(Arc::new(account_client(
            source_at(endpoint, None),
            tls_with(&[]).unwrap(),
        )))
        .unwrap();
        assert_calendar_reaches_the_delegate(&provider).await;
    }

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

    #[test]
    fn serves_every_calendar_with_enforced_writes_and_a_scope_each() {
        let account = AccountId::try_from("alice@gmail.com@mail.google.com").unwrap();
        let calendar = CalendarId::try_from("alice@gmail.com").unwrap();
        let provider = ReconnectingGoogleCalendarProvider::new(Arc::new(account_client(
            google_source(),
            tls_with(&[]).unwrap(),
        )))
        .unwrap();
        let info = provider.connection_info();
        assert!(info.capabilities.calendars());
        assert!(!info.capabilities.mail());
        assert_eq!(
            info.capabilities.calendar_write_guard(),
            Some(WriteGuard::Enforced),
        );
        // Reported before any request, as on Graph and for the same reason: `false` here would
        // have a host send its own iMIP reply on top of the one Google's `sendUpdates` already
        // sent.
        assert!(info.capabilities.calendar_scheduling());
        assert_eq!(
            provider.event_scope(&account, &calendar),
            SyncScope::GoogleCalendar {
                account: account.clone(),
                calendar,
            },
        );
        assert_eq!(
            provider.calendar_scope(&account),
            SyncScope::GoogleCalendarList { account },
        );
    }
}
