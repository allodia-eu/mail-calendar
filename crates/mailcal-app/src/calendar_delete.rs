//! Deleting an event, or one occurrence of a series, through the owning account's provider.

use engine_api::{LocalDateTime, Provider};

use crate::{
    App, CalendarWriteStatus,
    calendar_status::calendars_of,
    helpers::{generated_idempotency, now_utc},
    reference::EventRef,
};

impl<P: Provider> App<P> {
    /// Deletes the calendar `event` from its owning account via the engine's outbox (awaited
    /// inline; there is no background drainer yet), then rebuilds the agenda from the store.
    /// Routing by the reference's account is the fix:
    /// event keys are only unique within an account, so two accounts can mint the same key.
    ///
    /// `occurrence` names the single instance to remove, by its **original** start; `None`
    /// deletes the whole series.
    pub(crate) async fn delete_event(&self, event: EventRef, occurrence: Option<LocalDateTime>) {
        let Some(stored) = self.stored_event(&event).await else {
            return;
        };
        if let Some(named) = occurrence
            && !self.names_a_stored_occurrence(&event, &stored, named).await
        {
            // Refused rather than widened. A delete that could not find the occurrence and
            // removed the series instead is the one outcome nobody can undo.
            log::warn!(
                "delete_calendar_event: no such occurrence for {}",
                event.account.as_str()
            );
            self.set_calendar_write_status(CalendarWriteStatus::NotSaved);
            return;
        }
        let built = now_utc().and_then(|stamp| {
            mailcal_account::build_event_deletion(&stored, occurrence, stamp)
                .map_err(|err| err.to_string())
        });
        let deletion = match built {
            Ok(deletion) => deletion,
            Err(err) => {
                // Unlike the whole-series delete this cannot simply fall through: the user
                // asked for one occurrence to go, and returning quietly would leave it on
                // the grid with nothing anywhere saying the delete never happened.
                log::warn!(
                    "delete_calendar_event: the delete was not built for {}: {err}",
                    event.account.as_str()
                );
                self.set_calendar_write_status(CalendarWriteStatus::NotSaved);
                return;
            }
        };
        // Clone the account handle, then delete with the read guard released.
        let mut status = None;
        if let Some(acct) = self.account_handle(&event.account).await
            && let Some(provider) = acct.calendar_provider.as_ref()
        {
            self.set_calendar_write_status(CalendarWriteStatus::Saving);
            let delete = self
                .engine
                .delete_calendar_event(
                    provider,
                    &event.account,
                    &generated_idempotency(),
                    // The stored event, because removing **one occurrence** of a series is an
                    // edit of the stored document on CalDAV: the adapter rewrites those bytes
                    // and needs them. Passed for a whole-event delete too; a transport that
                    // deletes an object outright ignores it.
                    Some(&stored),
                    &deletion,
                )
                .await;
            status = Some(match delete {
                Ok(delete) => {
                    self.settle_calendar_write(
                        provider,
                        &event.account,
                        &calendars_of(&stored),
                        delete.reconciled,
                    )
                    .await
                }
                Err(err) => {
                    log::warn!(
                        "delete_calendar_event failed for {}: {err}",
                        event.account.as_str()
                    );
                    crate::connectivity::calendar_status(&err)
                }
            });
        }
        self.rebuild_calendar_view().await;
        // No adapter took the write: the calendar is not connected, and nothing was sent.
        self.set_calendar_write_status(status.unwrap_or(CalendarWriteStatus::NotSaved));
    }
}
