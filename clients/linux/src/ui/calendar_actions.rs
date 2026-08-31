//! Calendar actions and refresh coordination on the top-level Relm4 model.

use mailcal_bindings::{Intent, MailcalApp};

use super::{
    AppModel, PrimaryView,
    calendar::{EventForm, EventIdentity},
};
use crate::l10n;

/// Allows one retry after account discovery without coupling calendar sync to mail streaming.
#[derive(Default)]
pub(super) struct CalendarRefreshGate {
    mailbox_ready: bool,
    retry_pending: bool,
}

impl CalendarRefreshGate {
    pub(super) fn calendar_opened(&mut self) -> bool {
        if self.mailbox_ready {
            return true;
        }
        self.retry_pending = true;
        false
    }

    pub(super) fn mailbox_updated(
        &mut self,
        calendar_visible: bool,
        account_connected: bool,
    ) -> bool {
        if !account_connected {
            return false;
        }
        self.mailbox_ready = true;
        let retry_pending = std::mem::take(&mut self.retry_pending);
        calendar_visible && retry_pending
    }

    /// A session timer only reaches the provider after at least one account has connected.
    pub(super) const fn periodic_refresh(&self) -> bool {
        self.mailbox_ready
    }
}

impl AppModel {
    pub(super) fn show_calendar(&mut self) {
        self.primary = PrimaryView::Calendar;
        if self.calendar_refresh.calendar_opened() {
            self.dispatch(Intent::RefreshCalendar);
        }
    }

    pub(super) fn pull_mailbox(&mut self, app: &MailcalApp) {
        self.snapshot = app.mailbox_list();
        let account_connected = self
            .snapshot
            .accounts
            .iter()
            .any(|account| !app.connection_info(account.id.clone()).is_empty());
        if self
            .calendar_refresh
            .mailbox_updated(self.primary == PrimaryView::Calendar, account_connected)
        {
            app.dispatch(Intent::RefreshCalendar);
        }
        #[cfg(any(debug_assertions, feature = "dev-harness"))]
        self.apply_debug_open_hook();
    }

    pub(super) fn request_delete_event(&mut self, event: EventIdentity) {
        if let Some(app) = &self.app {
            self.calendar.request_delete(app, event);
        }
    }

    pub(super) fn submit_event(&mut self, form: &EventForm, this_occurrence_only: bool) {
        match self.calendar.submit_form(form, this_occurrence_only) {
            Ok(intent) => {
                self.dispatch(intent);
                self.calendar.dismiss_dialog();
            }
            Err(()) => self.notice = Some(l10n::event_editor_invalid().to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CalendarRefreshGate;

    #[test]
    fn mailbox_streaming_can_only_trigger_one_deferred_calendar_retry() {
        let mut gate = CalendarRefreshGate::default();
        assert!(!gate.calendar_opened());
        assert!(!gate.mailbox_updated(true, false));
        assert!(gate.mailbox_updated(true, true));
        assert!(!gate.mailbox_updated(true, true));
        assert!(!gate.mailbox_updated(true, true));
    }

    #[test]
    fn an_already_ready_mailbox_needs_no_deferred_retry() {
        let mut gate = CalendarRefreshGate::default();
        assert!(!gate.mailbox_updated(false, true));
        assert!(gate.calendar_opened());
        assert!(!gate.mailbox_updated(true, true));
    }

    #[test]
    fn the_session_timer_waits_for_an_account_then_keeps_refreshing() {
        let mut gate = CalendarRefreshGate::default();
        assert!(!gate.periodic_refresh());
        assert!(!gate.mailbox_updated(false, true));
        assert!(gate.periodic_refresh());
        assert!(gate.periodic_refresh());
    }
}
