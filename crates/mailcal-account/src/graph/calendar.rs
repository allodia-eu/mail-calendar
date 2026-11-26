//! Building the Microsoft Graph **calendar** provider a Microsoft account syncs its agenda
//! through: the calendar parallel of the mail providers in the parent module.
//!
//! One [`ReconnectingGraphCalendarProvider`] serves every calendar of the mailbox and forwards to
//! a [`GraphCalendarProvider`] on a client that asks the account's token source for the token on
//! each request. It carries the host's display zone, which the `Prefer: outlook.timezone` read
//! needs (see the engine's `graph.md` → Calendar). The date range `calendarView/delta` needs is
//! the engine's to name, on each fetch (`docs/calendar.md` §4).
//!
//! **Reads** (`sync_calendars` / `sync_events`) are idempotent GET/delta calls, so they get the
//! same one-shot reconnect loop the mail providers use. **Writes** (`create` / `patch` /
//! `delete`) are single-attempt: a blind retry of a non-idempotent `POST` could double-create,
//! and the app surfaces a failed write for the user to retry; one that met a dead connection
//! still replaces the client.

use std::sync::Arc;

use async_trait::async_trait;
use engine_api::{CalendarSync, CalendarWrites};
use engine_core::{
    calendar::{Calendar, Event},
    ids::{AccountId, CalendarId},
    sync::{SyncScope, SyncState, SyncUpdate},
    time::{Horizon, TimeZoneId},
};
use engine_provider::{
    CalendarListing, CalendarUserAddresses, ConnectionInfo, EventDeletion, EventDraft, EventEdit,
    EventRsvp, EventWriteReceipt, Provider, ProviderError, ProviderResult, ScopeSync,
    SharedAccount,
};
use provider_graph::{GraphCalendarProvider, GraphClient};

use super::{GraphTokenSource, account_client};
use crate::{
    AccountError,
    shared_client::{Delegate, SharedClient},
    tls::tls_with,
};

/// A [`Provider`] for every calendar of a Graph mailbox that forwards to a
/// [`GraphCalendarProvider`] and replaces its client once after a read fails on a dead
/// connection. Internal; the account layer hands callers `Box<dyn Provider>` from
/// [`connect_graph_calendar_provider`], never the concrete type.
#[derive(Debug)]
pub(crate) struct ReconnectingGraphCalendarProvider {
    delegate: Delegate<GraphClient, GraphCalendarProvider>,
}

impl ReconnectingGraphCalendarProvider {
    /// A calendar provider on `client`, reading event times in `display_zone`, the host's zone,
    /// sent as `Prefer: outlook.timezone` so Graph returns each event's wall clock in it and a
    /// recurring series expands DST-correctly.
    ///
    /// # Errors
    ///
    /// When the client cannot be built.
    pub(crate) fn new(
        client: Arc<SharedClient<GraphClient>>,
        display_zone: TimeZoneId,
    ) -> ProviderResult<Self> {
        Ok(Self {
            delegate: Delegate::new(client, move |client| {
                GraphCalendarProvider::new(client, display_zone.clone())
            })?,
        })
    }
}

/// The engine adapter's own answers, capabilities included: it states what a Graph series edit
/// costs the occurrences changed by hand, that Graph schedules server-side (so a host sends no
/// iMIP answer of its own beside Graph's), and that the engine names the window a snapshot opens
/// over.
#[async_trait]
impl Provider for ReconnectingGraphCalendarProvider {
    fn connection_info(&self) -> ConnectionInfo {
        self.delegate.last().connection_info()
    }

    /// Forwarded, because the adapter advertises `ByAddress`: Graph checks an address with a
    /// calendar probe.
    async fn resolve_shared_account(&self, address: &str) -> ProviderResult<SharedAccount> {
        self.delegate
            .retrying(|provider| async move { provider.resolve_shared_account(address).await })
            .await
    }
}

#[async_trait]
impl CalendarSync for ReconnectingGraphCalendarProvider {
    fn calendar_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::GraphCalendarList {
            account: account.clone(),
        }
    }

    fn event_scope(&self, account: &AccountId, calendar: &CalendarId) -> SyncScope {
        SyncScope::GraphCalendar {
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

impl engine_api::MailboxWrites for ReconnectingGraphCalendarProvider {}

#[async_trait]
impl CalendarWrites for ReconnectingGraphCalendarProvider {
    async fn create_event(
        &self,
        account: &AccountId,
        draft: &EventDraft,
    ) -> ProviderResult<EventWriteReceipt> {
        // A create is a non-idempotent `POST`; a blind retry could double-create, so this makes a
        // single attempt. A failure surfaces to the app.
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
        // `If-Match`-guarded, so a retry after a partial success risks a spurious `412`. Single
        // attempt, like `create_event`.
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

/// Connects the Graph calendar provider a Microsoft account syncs its agenda through: **one**
/// provider for every calendar of the mailbox, or of the shared store `shared` names, carrying
/// the host's `display_zone`. Reads the calendar list once first, so a token without the calendar
/// scope is refused here rather than on every pass.
///
/// The caller connects this only for its non-fatal effect: a calendar failure (an account whose
/// OAuth grant predates the `Calendars.ReadWrite` scope 403s here) leaves mail up with an empty
/// agenda.
///
/// # Errors
///
/// Returns [`AccountError`] if the initial token refresh or the calendar-list read fails.
pub async fn connect_graph_calendar_provider(
    account_id: &AccountId,
    tokens: Arc<GraphTokenSource>,
    display_zone: TimeZoneId,
    shared: Option<&engine_core::ids::SharedAccountId>,
) -> Result<Box<dyn Provider>, AccountError> {
    let tls = tls_with(&[])?;
    // A dead grant fails here as a sign-in to repeat, rather than as a calendar list that failed.
    tokens.access_token().await?;
    let client = account_client(tokens, super::principal(shared)?, tls);
    let provider = ReconnectingGraphCalendarProvider::new(client, display_zone)
        .map_err(|err| AccountError::Graph(err.to_string()))?;
    let calendars = list_calendars(account_id, &provider).await?;
    log::debug!(
        "graph: calendar list returned {} calendar(s)",
        calendars.len()
    );
    Ok(Box::new(provider))
}

/// Enumerates the account's calendars (a full snapshot) through `provider`: the parallel of the
/// mail side's `list_folders`.
async fn list_calendars(
    account_id: &AccountId,
    provider: &ReconnectingGraphCalendarProvider,
) -> Result<Vec<Calendar>, AccountError> {
    log::debug!("graph: fetching calendar list");
    let listing = provider
        .delegate
        .last()
        .sync_calendars(account_id, None)
        .await
        .map_err(|err| {
            // A `403` on the calendar-list probe is definitive: the token lacks the calendar
            // scope (an account connected before calendar support, or revoked consent), so the
            // user must re-authenticate: not a transient outage.
            if is_calendar_access_denied(&err) {
                AccountError::CalendarAccessDenied(err.to_string())
            } else {
                AccountError::Graph(err.to_string())
            }
        })?;
    Ok(match listing.sync.update {
        SyncUpdate::Snapshot { objects, .. } => objects,
        SyncUpdate::Delta { changed, .. } => changed,
    })
}

/// Whether a calendar-list probe error is an authorisation refusal (HTTP `403` /
/// `ErrorAccessDenied`) rather than a transient failure; i.e. the token lacks the calendar
/// scope. Matched on the provider error's rendered form (Graph surfaces both the status and the
/// `ErrorAccessDenied` code), the only place the HTTP status reaches this layer.
fn is_calendar_access_denied(err: &ProviderError) -> bool {
    let text = err.to_string();
    text.contains("403") || text.contains("ErrorAccessDenied")
}

#[cfg(test)]
#[path = "calendar_tests.rs"]
mod tests;
