//! Calendar sync entry points and their write-feedback policy.

use std::time::Instant;

use engine_api::Provider;

use crate::{
    App, CalendarWriteStatus,
    calendar_cache::rolling_horizon,
    calendar_unexpandable::unexpandable_line,
    sync_account::{Reach, reach_of_api, reachability, signin_expired},
};

impl<P: Provider> App<P> {
    /// Syncs every account's calendars and clears stale write feedback.
    ///
    /// This is the explicit retry behind [`crate::Intent::RefreshCalendar`]. Automatic refreshes
    /// use [`Self::refresh_calendar_in_background`] so they cannot dismiss a failed write.
    pub async fn refresh_calendar(&self) {
        self.refresh_calendar_with_write_feedback(true).await;
    }

    /// Syncs every account's calendars without changing the last write's feedback.
    ///
    /// Call this directly for launch, account-add and periodic refreshes. Dispatching an intent
    /// would record feature adoption even when the user never opened the calendar.
    pub async fn refresh_calendar_in_background(&self) {
        self.refresh_calendar_with_write_feedback(false).await;
    }

    async fn refresh_calendar_with_write_feedback(&self, clear_write_status: bool) {
        let Some(horizon) = rolling_horizon() else {
            return;
        };
        let zone = self.active_zone();
        let sync_start = Instant::now();
        // Every event the engine refuses to expand, from both passes. Such an event is stored
        // and the rest of the calendar materializes, so this is not a sync failure, but it
        // draws *nowhere*, and dropping the report is what makes that unreproducible.
        let mut refused = Vec::new();
        let accounts = self.account_handles().await;
        let (mut sources, mut synced_sources, mut failed) = (0, 0, 0);
        for (acct, account) in accounts.iter().enumerate() {
            let mut reaches = Vec::new();
            for (index, provider) in account.calendar_providers.iter().enumerate() {
                sources += 1;
                let started = Instant::now();
                let result = self
                    .engine
                    .sync_calendar(provider, &account.id, horizon, &zone)
                    .await;
                reaches.push(reach_of_api(&result));
                match result {
                    Ok(report) => {
                        synced_sources += 1;
                        refused.extend(report.events.unexpandable);
                    }
                    // Counted and logged, never surfaced as an error state: the calendars that
                    // did sync are still worth showing, and a failed write reaches the user
                    // through `CalendarWriteStatus` instead.
                    Err(error) => {
                        failed += 1;
                        log::warn!(
                            "calendar[a{acct}]: source[{index}] failed in {}ms: {error}",
                            started.elapsed().as_millis(),
                        );
                    }
                }
            }
            // An account with mail learns whether its server answers from its mail pass; one
            // without learns it here, or it could never say it is unreachable or signed out.
            if !account.uses_mail && !reaches.is_empty() {
                if let Some(reachable) = reachability(Reach::Busy, reaches.iter().copied()) {
                    self.set_account_reachable(&account.id, reachable);
                }
                self.apply_signin_expired(
                    &account.id,
                    signin_expired(Reach::Busy, reaches.into_iter()),
                );
            }
            // A delta expands only changed events. Re-expanding keeps a quiet account materialized
            // as the rolling window advances.
            match self
                .engine
                .expand_horizon(&account.id, horizon, &zone)
                .await
            {
                Ok(expansion) => refused.extend(expansion.unexpandable),
                Err(error) => log::warn!("calendar[a{acct}]: expansion failed: {error}"),
            }
        }
        let synced = sync_start.elapsed();
        let rebuild_start = Instant::now();
        let changed = self.rebuild_calendar_cache(horizon).await;
        if changed {
            self.rebuild_calendar().await;
        }
        if clear_write_status {
            self.set_calendar_write_status(CalendarWriteStatus::Idle);
        }
        log::info!(
            "refresh_calendar: {sources} source(s) on {} account(s); {synced_sources} synced, \
             {failed} failed; sync+expand {}ms + rebuild {}ms{}",
            accounts.len(),
            synced.as_millis(),
            rebuild_start.elapsed().as_millis(),
            if changed { "" } else { "; no redraw" }
        );
        if let Some(line) = unexpandable_line(&refused) {
            log::warn!("refresh_calendar: {line}");
        }
    }
}
