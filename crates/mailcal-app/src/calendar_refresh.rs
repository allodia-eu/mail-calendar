//! Calendar sync entry points and their write-feedback policy.

use std::time::Instant;

use engine_api::{AccountId, EventScopePass, Horizon, Provider, TimeZoneId, UnexpandableEvent};

use crate::{
    App, CalendarWriteStatus,
    accounts::Account,
    calendar_cache::rolling_horizon,
    calendar_unexpandable::unexpandable_line,
    sync_account::{reach_of, reach_of_api, reachability, signin_expired},
};

/// What one account's calendar pass did, for the refresh's summary line.
#[derive(Default)]
struct CalendarPass {
    calendars: usize,
    synced: usize,
    unchanged: usize,
    busy: usize,
    failed: usize,
    /// Every event the engine refused to expand. Such an event is stored and the rest of the
    /// calendar materialises, so this is not a sync failure, but it draws *nowhere*, and dropping
    /// the report is what makes that unreproducible.
    refused: Vec<UnexpandableEvent>,
}

impl<P: Provider> App<P> {
    /// Syncs every account's calendars and clears stale write feedback.
    ///
    /// This is the explicit retry behind [`crate::Intent::RefreshCalendar`]. Automatic refreshes
    /// use [`Self::refresh_calendar_in_background`] so they cannot dismiss a failed write.
    pub async fn refresh_calendar(&self) {
        self.refresh_calendars(None, true).await;
    }

    /// Syncs every account's calendars without changing the last write's feedback.
    ///
    /// Call this directly for a periodic refresh. Dispatching an intent would record feature
    /// adoption even when the user never opened the calendar.
    pub async fn refresh_calendar_in_background(&self) {
        self.refresh_calendars(None, false).await;
    }

    /// [`Self::refresh_calendar_in_background`], syncing the calendars of the accounts in `ids`
    /// alone: the ones a change reached (added, signed in again, given a calendar, reconnected).
    /// Every other account is left alone, each being a round trip and a scope lease. Every
    /// account is still re-expanded and the cache rebuilt, which costs no network, so the
    /// rolling window advances on a pass that connected nothing.
    pub async fn refresh_calendar_of(&self, ids: &[AccountId]) {
        self.refresh_calendars(Some(ids), false).await;
    }

    async fn refresh_calendars(&self, only: Option<&[AccountId]>, clear_write_status: bool) {
        let Some(horizon) = rolling_horizon() else {
            return;
        };
        let zone = self.active_zone();
        let sync_start = Instant::now();
        let accounts = self.account_handles().await;
        // Accounts are independent servers, so they sync at once rather than in turn.
        let passes =
            futures::future::join_all(accounts.iter().enumerate().map(|(acct, account)| {
                let sync = only.is_none_or(|ids| ids.contains(&account.id));
                self.refresh_account_calendar(acct, account, sync, horizon, &zone)
            }))
            .await;
        let synced = sync_start.elapsed();
        let rebuild_start = Instant::now();
        let changed = self.rebuild_calendar_cache(horizon).await;
        if changed {
            self.rebuild_calendar().await;
        }
        if clear_write_status {
            self.set_calendar_write_status(CalendarWriteStatus::Idle);
        }
        let mut total = CalendarPass::default();
        for pass in passes {
            total.calendars += pass.calendars;
            total.synced += pass.synced;
            total.unchanged += pass.unchanged;
            total.busy += pass.busy;
            total.failed += pass.failed;
            total.refused.extend(pass.refused);
        }
        log::info!(
            "refresh_calendar: {} calendar(s); {} synced, {} unchanged, {} busy, {} failed; \
             sync+expand {}ms + rebuild {}ms{}",
            total.calendars,
            total.synced,
            total.unchanged,
            total.busy,
            total.failed,
            synced.as_millis(),
            rebuild_start.elapsed().as_millis(),
            if changed { "" } else { "; no redraw" }
        );
        if let Some(line) = unexpandable_line(&total.refused) {
            log::warn!("refresh_calendar: {line}");
        }
    }

    /// Syncs one account's calendars when `sync` asks for it, then re-expands them.
    async fn refresh_account_calendar(
        &self,
        acct: usize,
        account: &Account<P>,
        sync: bool,
        horizon: Horizon,
        zone: &TimeZoneId,
    ) -> CalendarPass {
        let mut pass = CalendarPass::default();
        if let Some(provider) = account.calendar_provider.as_ref().filter(|_| sync) {
            let started = Instant::now();
            let result = self
                .engine
                .sync_calendar(provider, &account.id, horizon, zone)
                .await;
            let list = reach_of_api(&result);
            let mut reaches = Vec::new();
            match result {
                Ok(report) => {
                    pass.calendars = report.scopes.len();
                    for (index, scope) in report.scopes.iter().enumerate() {
                        reaches.push(reach_of(&scope.result));
                        match &scope.result {
                            Ok(EventScopePass::Unchanged) => pass.unchanged += 1,
                            Ok(EventScopePass::Synced(_)) => pass.synced += 1,
                            // Another pass holds the scope and brings the same changes.
                            Err(error) if error.is_busy() => pass.busy += 1,
                            // Counted and logged, never surfaced as an error state: the
                            // calendars that did sync are still worth showing, and a failed
                            // write reaches the user through `CalendarWriteStatus` instead.
                            Err(error) => {
                                pass.failed += 1;
                                log::warn!("calendar[a{acct}]: calendar[{index}] failed: {error}");
                            }
                        }
                    }
                    pass.refused.extend(report.events.unexpandable);
                }
                Err(engine_api::ApiError::Busy) => {
                    pass.busy += 1;
                    log::debug!("calendar[a{acct}]: busy, another pass holds the calendar list");
                }
                Err(error) => {
                    pass.failed += 1;
                    log::warn!(
                        "calendar[a{acct}]: calendar list failed in {}ms: {error}",
                        started.elapsed().as_millis(),
                    );
                }
            }
            // An account with mail learns whether its server answers from its mail pass; one
            // without learns it here, or it could never say it is unreachable or signed out.
            if !account.uses_mail {
                if let Some(reachable) = reachability(list, reaches.iter().copied()) {
                    self.set_account_reachable(&account.id, reachable);
                }
                self.apply_signin_expired(&account.id, signin_expired(list, reaches.into_iter()));
            }
        }
        // A delta expands only changed events. Re-expanding keeps a quiet account materialised
        // as the rolling window advances.
        match self.engine.expand_horizon(&account.id, horizon, zone).await {
            Ok(expansion) => pass.refused.extend(expansion.unexpandable),
            Err(error) => log::warn!("calendar[a{acct}]: expansion failed: {error}"),
        }
        pass
    }
}
