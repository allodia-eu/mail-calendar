//! Calendar sync entry points and their write-feedback policy.

use std::time::Instant;

use engine_api::Provider;

use crate::{
    App, CalendarWriteStatus, calendar_cache::rolling_horizon,
    calendar_unexpandable::unexpandable_line,
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
        for account in self.account_handles().await {
            for provider in &account.calendar_providers {
                if let Ok(report) = self
                    .engine
                    .sync_calendar(provider, &account.id, horizon, &zone)
                    .await
                {
                    refused.extend(report.events.unexpandable);
                }
            }
            // A delta expands only changed events. Re-expanding keeps a quiet account materialized
            // as the rolling window advances.
            if let Ok(expansion) = self
                .engine
                .expand_horizon(&account.id, horizon, &zone)
                .await
            {
                refused.extend(expansion.unexpandable);
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
            "refresh_calendar: sync+expand {}ms + rebuild {}ms{}",
            synced.as_millis(),
            rebuild_start.elapsed().as_millis(),
            if changed { "" } else { "; no redraw" }
        );
        if let Some(line) = unexpandable_line(&refused) {
            log::warn!("refresh_calendar: {line}");
        }
    }
}
