//! The event-detail read on [`App`]: fetch the stored event through the `engine-api` facade and
//! project it. The projection itself, which reaches into `engine-core` domain types (a
//! reminder's trigger, a recurrence frequency); lives in [`mailcal_account`], the
//! provider-format glue layer, so this app-layer module stays on the facade like the rest of
//! `mailcal-app`.

use std::time::Instant;

use engine_api::Provider;
use mailcal_account::{
    EventDetail, EventEdit, SeriesEditWarning, project_event_detail, series_edit_touches,
    series_edit_warning,
};

use crate::{App, reference::EventRef};

impl<P: Provider> App<P> {
    /// The full detail of the stored event `event` names, or `None` if it is not in the store
    /// (a torn read, or a stale reference). A local read: no network, no expansion.
    ///
    /// `occurrence` is the token of the instance the user opened, when they opened one rather
    /// than the series. Resolving it is what makes the times the *occurrence's*: a series' own
    /// start is its **first** occurrence's, so a detail projected without this reads September's
    /// standup as August's, and an editor prefilled from it would write that date back.
    ///
    /// Times itself: this is the tap-to-open path, and it used to scan the account's entire event
    /// history to find one key, so on a large diary it stalled for seconds; with no log line to
    /// show it. The duration (never a title, time, or attendee: the never-log-content rule holds)
    /// makes a regression to that behaviour visible instead of merely felt.
    pub async fn event_detail(
        &self,
        event: &EventRef,
        occurrence: Option<&str>,
    ) -> Option<EventDetail> {
        let started = Instant::now();
        let stored = self.stored_event(event).await?;
        let account = self.account_handle(&event.account).await;
        let can_write = account
            .as_ref()
            .is_some_and(|account| Self::account_can_write(account));
        let at = match occurrence.filter(|token| !token.is_empty()) {
            Some(token) => self.resolve_occurrence(event, &stored, token).await,
            None => None,
        };
        let detail = project_event_detail(event.account.as_str(), &stored, can_write, at.as_ref());
        log::info!(
            "event_detail: resolved in {}ms",
            started.elapsed().as_millis()
        );
        Some(detail)
    }

    /// What saving `edit` over the **whole series** would cost the occurrences the user changed
    /// individually, or `None` when there is nothing to say.
    ///
    /// Asked with the edit in hand, which is the only moment all three facts exist: the server's
    /// policy, whether this series holds any per-occurrence work, and which of the policy's
    /// clauses this particular change triggers. A detail cannot answer it; it is projected
    /// before the user has touched the form, so it would have to assume the worst edit and
    /// announce losses that will not happen.
    ///
    /// An edit scoped to **one occurrence** is never warned about and must not ask: it writes an
    /// override of its own and leaves every other occurrence alone.
    pub async fn series_edit_warning(
        &self,
        event: &EventRef,
        edit: &EventEdit,
    ) -> Option<SeriesEditWarning> {
        if edit.occurrence.is_some() {
            return None;
        }
        let stored = self.stored_event(event).await?;
        let account = self.account_handle(&event.account).await;
        // A post-connect fact about the *server*, so it is read here rather than in the pure
        // decision below.
        let survival = account
            .as_ref()
            .and_then(|account| account.calendar_providers.first())
            .and_then(|provider| provider.connection_info().capabilities.override_survival());
        series_edit_warning(
            survival,
            stored
                .recurrence
                .as_ref()
                .is_some_and(|recurrence| !recurrence.overrides.is_empty()),
            series_edit_touches(&stored, edit),
        )
    }
}
