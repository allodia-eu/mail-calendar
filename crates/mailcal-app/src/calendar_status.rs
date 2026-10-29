//! Calendar-write status: turning a write's [`Reconciled`] outcome into the
//! [`CalendarWriteStatus`] the host shows, and the status accessors. Split out of
//! `calendar_ops.rs` to keep each file under the 500-line limit; an `impl App` block here
//! reuses the runtime's fields, and the write ops in `calendar_ops.rs` call back into it.

use engine_api::{AccountId, ApiError, CalendarId, Provider, Reconciled};

use crate::{App, CalendarWriteStatus, Surface};

impl<P: Provider> App<P> {
    /// Turns a write's [`Reconciled`] outcome into the [`CalendarWriteStatus`] the host shows,
    /// recovering from a contended reconcile rather than reporting it as a lost write.
    ///
    /// The one dangerous mistake here is re-issuing the write, so this never does. Anything but
    /// [`Reconciled::Applied`] means the write **already landed on the server** and only the
    /// local copy is stale, so recovery is a *re-read* ([`Engine::reconcile_calendar_events`]),
    /// not a re-write:
    ///
    /// - `Applied`: the store holds the server's copy. [`Saved`](CalendarWriteStatus::Saved).
    /// - `Busy`: a concurrent sync held the event scope. Re-read once; if that too is `Busy` the
    ///   in-flight sync will settle it, so the write is still
    ///   [`Saved`](CalendarWriteStatus::Saved). A re-read error we cannot classify away leaves it
    ///   [`Unconfirmed`](CalendarWriteStatus::Unconfirmed).
    /// - `Failed`: the post-write delta failed. The change is on the server but the local view is
    ///   unconfirmed, so we surface [`Unconfirmed`](CalendarWriteStatus::Unconfirmed), *not* "your
    ///   change was not saved"; the next full sync heals the store.
    ///
    /// `calendars` are the ones the write touched: the re-read covers those and no other.
    pub(crate) async fn settle_calendar_write<Prov: Provider>(
        &self,
        provider: &Prov,
        account: &AccountId,
        calendars: &[CalendarId],
        reconciled: Reconciled,
    ) -> CalendarWriteStatus {
        let account_handle = mailcal_account::account_log_handle(account.as_str());
        match reconciled {
            Reconciled::Applied(_) => {
                log::info!("calendar write reconciled for [{account_handle}]");
                CalendarWriteStatus::Saved
            }
            Reconciled::Busy => {
                log::info!(
                    "calendar write busy for [{account_handle}]; re-reading the event scope"
                );
                let mut status = CalendarWriteStatus::Saved;
                for calendar in calendars {
                    match self
                        .engine
                        .reconcile_calendar_events(provider, account, calendar)
                        .await
                    {
                        Ok(_) => {}
                        Err(ApiError::Busy) => log::info!(
                            "reconcile still busy for [{account_handle}]; the in-flight sync will settle it"
                        ),
                        Err(err) => {
                            log::warn!("post-write reconcile failed for [{account_handle}]: {err}");
                            status = CalendarWriteStatus::Unconfirmed;
                        }
                    }
                }
                status
            }
            Reconciled::Failed(err) => {
                log::warn!("calendar write reconciliation failed for [{account_handle}]: {err}");
                CalendarWriteStatus::Unconfirmed
            }
            _ => {
                log::warn!("unknown calendar write reconciliation result for [{account_handle}]");
                CalendarWriteStatus::Unconfirmed
            }
        }
    }

    /// The most recent calendar write's status (pulled after a [`Surface::CalendarStatus`]
    /// signal). See [`CalendarWriteStatus`]; in particular, `Unconfirmed` does not mean the save
    /// was lost.
    #[must_use]
    pub fn calendar_write_status(&self) -> CalendarWriteStatus {
        *self
            .calendar_write_status
            .lock()
            .expect("calendar-write-status mutex poisoned")
    }

    /// Sets the calendar-write status, signalling [`Surface::CalendarStatus`] only when the
    /// value actually changes: so a steady stream of writes doesn't churn the host, and a
    /// full refresh that clears a stale failure fires exactly once.
    pub(crate) fn set_calendar_write_status(&self, status: CalendarWriteStatus) {
        let mut current = self
            .calendar_write_status
            .lock()
            .expect("calendar-write-status mutex poisoned");
        if *current != status {
            *current = status;
            drop(current);
            self.observer.surface_changed(Surface::CalendarStatus);
        }
    }

    /// Settles a calendar write the host sent but this core could not read, such as an invitee
    /// that is not an address. The editor that sent it waits on this status, so the write ends as
    /// [`CalendarWriteStatus::NotSaved`] rather than vanishing.
    pub fn refuse_calendar_write(&self) {
        self.set_calendar_write_status(CalendarWriteStatus::NotSaved);
    }

    /// The reason `error` stopped a calendar answer, settling it as
    /// [`Unconfirmed`](CalendarWriteStatus::Unconfirmed) where the write may have landed. The
    /// answer's own wrapper settles every other failure as not saved.
    pub(crate) fn answer_write_failed(&self, error: &ApiError) -> String {
        if error.may_have_applied() {
            self.set_calendar_write_status(CalendarWriteStatus::Unconfirmed);
        }
        error.to_string()
    }

    /// Settles a calendar answer that did not happen: not saved, unless its write may have
    /// landed and already said so.
    pub(crate) fn answer_failed(&self) {
        if self.calendar_write_status() != CalendarWriteStatus::Unconfirmed {
            self.set_calendar_write_status(CalendarWriteStatus::NotSaved);
        }
    }
}

/// The calendars a stored event belongs to: the ones a write to it touches.
pub(crate) fn calendars_of(event: &engine_api::Event) -> Vec<engine_api::CalendarId> {
    event.calendars.iter().cloned().collect()
}
