//! What each surface signal makes the host pull back out of the core.
//!
//! Split from [`super::update`], the input dispatcher, so each file stays under the 500-line
//! limit: that one decides what the host *asks* for, this one what it *reads* when the core says
//! something changed.

use mailcal_bindings::{SendStatus, Surface};

use super::{AppModel, sync_line};
use crate::l10n;

impl AppModel {
    pub(super) fn pull(&mut self, surface: &Surface) {
        let Some(app) = self.app.clone() else {
            return;
        };
        match surface {
            Surface::MailboxList => self.pull_mailbox(&app),
            Surface::Reading => {
                self.reading.snapshot = app.reading_view();
                // The signal says *some* reader's body changed, never which, so every open window
                // reads its own slot back beside the pane (`docs/reading-window.md`).
                self.reload_reading_windows(&app);
                self.reading_generation = self.reading_generation.wrapping_add(1);
            }
            Surface::Sending => {
                self.notice = match app.send_status() {
                    SendStatus::Sending => Some(l10n::send_status_sending().to_owned()),
                    SendStatus::Sent => Some(l10n::send_status_sent().to_owned()),
                    // Not a failure: the message waits in the Outbox and goes out by itself.
                    // The passing form of what the pane's Outbox row says for as long as the
                    // message is waiting.
                    SendStatus::Queued => Some(l10n::send_status_queued().to_owned()),
                    // Not a failure either, and not a wait: it may have been delivered, and
                    // the Outbox row asks whether it was.
                    SendStatus::Unconfirmed => Some(l10n::send_status_unconfirmed().to_owned()),
                    // Refused, and kept: the hint says where the message is now.
                    SendStatus::NotSent => Some(l10n::send_status_not_sent().to_owned()),
                    SendStatus::Failed => Some(l10n::send_status_failed().to_owned()),
                    // Nothing to show, for two different reasons: nothing is in flight, and for
                    // `SentNotFiled` the standing notice already says it: with its answers, which
                    // a transient hint has nowhere to put.
                    SendStatus::Idle | SendStatus::SentNotFiled => None,
                };
            }
            // A message the core withdrew from the Outbox so the user could change it. It
            // exists nowhere else by the time this arrives, so this may not refuse.
            Surface::ComposeRequest => {
                if let Some(opening) = app.compose_request() {
                    self.open_withdrawn_message(&app, opening);
                }
            }
            // All three progress surfaces share one snapshot and one strip. The foreground bar
            // wins while active; below it the caption is either an account being made to wait
            // or a pass nobody started.
            Surface::SyncProgress => {
                let progress = app.sync_progress();
                self.sync_bar = sync_line::sync_bar(&progress);
                self.sync_status = sync_line::sync_status(&progress, &self.snapshot.accounts);
            }
            Surface::Calendar => self.calendar.refresh(&app),
            Surface::Contacts => self.contacts.refresh(&app),
            Surface::ContactsStatus => self.contacts.refresh_write_status(&app),
            Surface::CalendarStatus => self.calendar.refresh_write_status(&app),
            Surface::Settings => {
                self.calendar.refresh_settings(&app);
                self.settings.signalled();
                // After the signal's redraw, so a notice it raises is drawn rather than skipped.
                self.account_work_signalled();
                // A link suggestion the core found after the link step was drawn.
                self.refresh_link_step();
                self.server_change_signalled();
            }
            // A composition's save moved. The signal names none, so every open composer reads
            // its own state back (`docs/drafts.md`).
            Surface::DraftStatus => self.pull_draft_status(),
            // Read on its own: nothing heavier is reloaded for it. What an account's row in
            // Settings says can move on this signal too, and Accounts redraws only when what it
            // drew changed.
            Surface::Notices => {
                self.settings.signalled();
                self.pull_notices();
                // A lookup that replaced the question on screen arrives as a connectivity fact.
                self.server_change_signalled();
            }
        }
    }
}
