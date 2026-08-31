//! Building the Google **calendar** provider a Google account syncs its agenda through, with
//! automatic OAuth token refresh: the calendar parallel of the Gmail provider in the parent
//! module, and the Google sibling of [`crate::graph::connect_graph_calendar_providers`].
//!
//! Like the mail side, the engine's [`GoogleCalendarProvider`] takes a **static** bearer token,
//! so this wraps it in a [`RefreshingGoogleCalendarProvider`] that mints a fresh access token
//! before each network call and delegates to a freshly built provider (reusing its warm
//! connection pool while the token is unchanged). The provider is bound to **one** calendar;
//! the account's primary; carrying a fetch window centred on today, re-centred each reconnect.
//!
//! Two things are simpler than the Graph calendar: Google is **IANA-native** (event times carry
//! an IANA `timeZone`), so there is **no display-zone `Prefer` header** to thread through, and
//! Google requests the calendar scope at connect time alongside mail, so there is no
//! "connected before calendar support" re-consent case: a calendar failure is simply
//! non-fatal (mail stays up with an empty agenda).
//!
//! **Reads** (`sync_calendars`/`sync_events`) are idempotent, so they get the rate-limit backoff
//! plus the one-shot reconnect loop the mail provider uses. **Writes** (`create`/`patch`/`delete`)
//! are single-attempt (a blind retry of a non-idempotent call could double-create) and a failed
//! write surfaces to the app to retry.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use engine_core::{
    calendar::{Calendar, Event},
    ids::{AccountId, CalendarId},
    sync::{SyncScope, SyncState, SyncUpdate},
};
use engine_provider::{
    Capabilities, ConnectionInfo, EventDeletion, EventDraft, EventEdit, EventWriteReceipt,
    OverrideSurvival, Provider, ProviderError, ProviderResult, ScopeSync, WriteGuard,
};
use engine_tls::TlsClientConfig;
use provider_google::{CalendarWindow, GoogleCalendarProvider, GoogleClient};
use time::{Duration, OffsetDateTime};

use super::{calendar_date, should_reconnect};
use crate::{AccountError, GraphTokenSource, throttle::account_retry, tls::account_tls};

/// What a Google series edit costs the occurrences the user changed by hand: moving the
/// series' time destroys them, and renaming the series renames the one they had renamed.
/// Only a rule change leaves them alone.
///
/// The **fallback** copy of the engine adapter's own constant, for the window before a
/// delegate exists; see the Graph sibling for why the fallback has to be truthful.
const GOOGLE_OVERRIDE_SURVIVAL: OverrideSurvival = OverrideSurvival {
    survives_time_change: false,
    survives_rule_change: true,
    clobbers_own_fields: true,
};

/// How many days **back** the baked-in fetch window spans (the app's rolling horizon), so a
/// freshly connected provider already covers everything the grid can scroll to.
const WINDOW_DAYS_BACK: i64 = 120;

/// How many days **ahead** the baked-in fetch window spans (the app's rolling horizon).
const WINDOW_DAYS_AHEAD: i64 = 400;

/// A [`Provider`] bound to one Google calendar that refreshes its access token before every
/// network call and delegates to a freshly built [`GoogleCalendarProvider`]. Internal; the
/// account layer hands callers `Box<dyn Provider>` from
/// [`connect_google_calendar_providers`].
#[derive(Debug)]
pub(crate) struct RefreshingGoogleCalendarProvider {
    /// The calendar this provider syncs and writes to (the account's primary).
    calendar: CalendarId,
    /// The shared, self-refreshing token source, shared with the account's Gmail provider so
    /// one refresh serves both.
    tokens: Arc<GraphTokenSource>,
    /// Calendar read + a server-enforced write guard; Google rejects a stale `If-Match` with a
    /// `412`, exactly like Graph.
    capabilities: Capabilities,
    /// The initial (snapshot) event window, centred on today at connect time.
    window: CalendarWindow,
    /// The account's shared TLS policy, cloned into each rebuilt Google client.
    tls: TlsClientConfig,
    /// The built delegate, cached by the access token it was built with, so the reqwest
    /// connection pool is reused; rebuilt only when the token refreshes.
    cached: Mutex<Option<(String, Arc<GoogleCalendarProvider>)>>,
}

impl RefreshingGoogleCalendarProvider {
    /// Binds a refreshing calendar provider to `calendar`, sharing `tokens` with the account's
    /// Gmail provider.
    #[must_use]
    pub(crate) fn new(
        calendar: CalendarId,
        tokens: Arc<GraphTokenSource>,
        window: CalendarWindow,
        tls: TlsClientConfig,
    ) -> Self {
        Self {
            calendar,
            tokens,
            capabilities: Capabilities::none()
                .with_calendars()
                .with_calendar_writes(WriteGuard::Enforced, GOOGLE_OVERRIDE_SURVIVAL)
                // Google schedules server-side (`sendUpdates`), so the answer travels by
                // itself. As on Graph this is only the pre-delegate fallback, and it matters
                // for the same reason: a host told otherwise would send its own iMIP reply
                // beside the one the server already sent.
                .with_calendar_scheduling(),
            window,
            tls,
            cached: Mutex::new(None),
        }
    }

    /// Returns the delegate `GoogleCalendarProvider`, reusing the cached one while the access
    /// token is unchanged and rebuilding it (and its warm connection pool) only on a refresh.
    async fn delegate(&self) -> ProviderResult<Arc<GoogleCalendarProvider>> {
        let token = self.tokens.access_token().await.map_err(|err| match err {
            AccountError::SigninRejected(detail) => ProviderError::authentication(detail),
            other => ProviderError::retryable(other.to_string()),
        })?;
        {
            let cache = self
                .cached
                .lock()
                .expect("google calendar delegate mutex poisoned");
            if let Some((cached_token, provider)) = cache.as_ref()
                && *cached_token == token
            {
                return Ok(Arc::clone(provider));
            }
        }
        let client = GoogleClient::connect(token.clone(), &self.tls, &account_retry())
            .map_err(ProviderError::from)?;
        let provider = Arc::new(
            GoogleCalendarProvider::new(client, self.calendar.clone()).with_window(self.window),
        );
        *self
            .cached
            .lock()
            .expect("google calendar delegate mutex poisoned") =
            Some((token, Arc::clone(&provider)));
        Ok(provider)
    }

    /// Drops the cached delegate so the next [`delegate`](Self::delegate) rebuilds a fresh client
    /// ; used after a retryable transport failure whose keep-alive socket is presumed dead.
    fn invalidate_delegate(&self) {
        *self
            .cached
            .lock()
            .expect("google calendar delegate mutex poisoned") = None;
    }
}

#[async_trait]
impl Provider for RefreshingGoogleCalendarProvider {
    fn connection_info(&self) -> ConnectionInfo {
        self.cached
            .lock()
            .expect("google calendar delegate mutex poisoned")
            .as_ref()
            .map_or_else(
                || ConnectionInfo::new(self.capabilities),
                |(_, provider)| provider.connection_info(),
            )
    }

    fn calendar_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::GoogleCalendarList {
            account: account.clone(),
        }
    }

    fn event_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::GoogleCalendar {
            account: account.clone(),
            calendar: self.calendar.clone(),
        }
    }

    async fn sync_calendars(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Calendar>> {
        let mut provider = self.delegate().await?;
        let mut reconnected = false;
        loop {
            match provider.sync_calendars(account, cursor).await {
                Ok(value) => return Ok(value),
                Err(err) if !reconnected && should_reconnect(&err) => {
                    self.invalidate_delegate();
                    provider = self.delegate().await?;
                    reconnected = true;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn sync_events(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Event>> {
        let mut provider = self.delegate().await?;
        let mut reconnected = false;
        loop {
            match provider.sync_events(account, cursor).await {
                Ok(value) => return Ok(value),
                Err(err) if !reconnected && should_reconnect(&err) => {
                    self.invalidate_delegate();
                    provider = self.delegate().await?;
                    reconnected = true;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn create_event(
        &self,
        account: &AccountId,
        draft: &EventDraft,
    ) -> ProviderResult<EventWriteReceipt> {
        // A create is non-idempotent; a blind retry could double-create, so single attempt.
        let provider = self.delegate().await?;
        provider.create_event(account, draft).await
    }

    async fn patch_event(
        &self,
        account: &AccountId,
        base: &Event,
        edit: &EventEdit,
    ) -> ProviderResult<EventWriteReceipt> {
        // `If-Match`-guarded, so a retry after a partial success risks a spurious `412`.
        let provider = self.delegate().await?;
        provider.patch_event(account, base, edit).await
    }

    async fn delete_event(
        &self,
        account: &AccountId,
        base: Option<&Event>,
        deletion: &EventDeletion,
    ) -> ProviderResult<()> {
        let provider = self.delegate().await?;
        provider.delete_event(account, base, deletion).await
    }
}

/// Connects the Google calendar provider a Google account syncs its agenda through: **one**
/// token-refreshing provider bound to the account's **primary** calendar: the Google parallel
/// of [`connect_graph_calendar_providers`](crate::connect_graph_calendar_providers). Enumerates
/// `calendarList` once with a fresh token, picks the primary (falling back to the first), and
/// binds a shared-token refreshing provider carrying a fetch window centred on today.
///
/// The caller connects this only for its non-fatal effect: a calendar failure leaves mail up
/// with an empty agenda.
///
/// # Errors
///
/// Returns [`AccountError`] if the initial token refresh or the calendar-list sync fails, or the
/// account has no calendar at all.
pub async fn connect_google_calendar_providers(
    account_id: &AccountId,
    tokens: Arc<GraphTokenSource>,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    let tls = account_tls()?;
    let window = rolling_window()?;
    let calendars = list_calendars(account_id, &tokens, &tls, window).await?;
    let calendar = calendars
        .iter()
        .find(|calendar| calendar.is_default)
        .or_else(|| calendars.first())
        .map(|calendar| calendar.id.clone())
        .ok_or_else(|| AccountError::Google("account has no calendar".to_owned()))?;
    Ok(vec![Box::new(RefreshingGoogleCalendarProvider::new(
        calendar, tokens, window, tls,
    ))])
}

/// Enumerates the account's calendars (a full snapshot) with a one-off client on a fresh access
/// token: the parallel of the mail side's implicit label-list sync.
async fn list_calendars(
    account_id: &AccountId,
    tokens: &Arc<GraphTokenSource>,
    tls: &TlsClientConfig,
    window: CalendarWindow,
) -> Result<Vec<Calendar>, AccountError> {
    let token = tokens.access_token().await?;
    let client = GoogleClient::connect(token, tls, &account_retry())
        .map_err(|err| AccountError::Google(err.to_string()))?;
    // The `calendarList` call ignores the bound calendar, so any placeholder id serves to
    // construct the probe (the `primary` alias Google always accepts).
    let placeholder =
        CalendarId::try_from("primary").map_err(|err| AccountError::Google(err.to_string()))?;
    log::debug!("google: fetching calendar list");
    let listing = GoogleCalendarProvider::new(client, placeholder)
        .with_window(window)
        .sync_calendars(account_id, None)
        .await
        .map_err(|err| AccountError::Google(err.to_string()))?;
    let calendars = match listing.update {
        SyncUpdate::Snapshot { objects, .. } => objects,
        SyncUpdate::Delta { changed, .. } => changed,
    };
    log::debug!(
        "google: calendar list returned {} calendar(s)",
        calendars.len()
    );
    Ok(calendars)
}

/// The event fetch window, centred on today: a few months back and a bit over a year ahead,
/// matching the app's rolling calendar horizon.
fn rolling_window() -> Result<CalendarWindow, AccountError> {
    let today = OffsetDateTime::now_utc().date();
    let start = calendar_date(today - Duration::days(WINDOW_DAYS_BACK))
        .ok_or_else(|| AccountError::Google("calendar window start out of range".to_owned()))?;
    let end = calendar_date(today + Duration::days(WINDOW_DAYS_AHEAD))
        .ok_or_else(|| AccountError::Google("calendar window end out of range".to_owned()))?;
    Ok(CalendarWindow::new(start, end))
}

#[cfg(test)]
mod tests {
    use mailcal_oauth::{GOOGLE_SCOPES, OAuthClient, OAuthProviderConfig};

    use super::*;

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
    fn binds_a_calendar_provider_with_enforced_writes_and_the_bound_scopes() {
        let account = AccountId::try_from("alice@gmail.com@mail.google.com").unwrap();
        let calendar = CalendarId::try_from("primary").unwrap();
        let provider = RefreshingGoogleCalendarProvider::new(
            calendar.clone(),
            google_source(),
            rolling_window().unwrap(),
            account_tls().unwrap(),
        );
        let info = provider.connection_info();
        assert!(info.capabilities.calendars());
        assert!(!info.capabilities.mail());
        assert_eq!(
            info.capabilities.calendar_write_guard(),
            Some(WriteGuard::Enforced),
        );
        // As on Graph, and for the same reason: `false` here would have a host send its own
        // iMIP reply on top of the one Google's `sendUpdates` already sent.
        assert!(info.capabilities.calendar_scheduling());
        assert_eq!(
            provider.event_scope(&account),
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

    #[test]
    fn the_fetch_window_brackets_today() {
        let today = OffsetDateTime::now_utc().date();
        let window = rolling_window().unwrap();
        assert_eq!(
            window.start,
            calendar_date(today - Duration::days(WINDOW_DAYS_BACK)).unwrap(),
        );
        assert_eq!(
            window.end,
            calendar_date(today + Duration::days(WINDOW_DAYS_AHEAD)).unwrap(),
        );
    }
}
