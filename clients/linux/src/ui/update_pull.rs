//! What each surface signal makes the host pull back out of the core.
//!
//! Split from [`super::update`], the input dispatcher, so each file stays under the 500-line
//! limit: that one decides what the host *asks* for, this one what it *reads* when the core says
//! something changed.

use mailcal_bindings::{SendStatus, Surface};

use super::{AppModel, connectivity::ConnectivityState, model, unfiled_copy::UnfiledCopyNotice};
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
                #[cfg(any(debug_assertions, feature = "dev-harness"))]
                self.continue_showcase();
            }
            Surface::Sending => {
                self.notice = match app.send_status() {
                    SendStatus::Sending => Some(l10n::send_status_sending().to_owned()),
                    SendStatus::Sent => Some(l10n::send_status_sent().to_owned()),
                    // Not a failure: the message waits in the Outbox and goes out by
                    // itself. This client draws no Outbox yet (`docs/sending.md` → Known
                    // gaps), so the hint is the only thing that says so.
                    SendStatus::Queued => Some(l10n::send_status_queued().to_owned()),
                    SendStatus::Failed => Some(l10n::send_status_failed().to_owned()),
                    // Nothing to show, for two different reasons: nothing is in flight, and for
                    // `SentNotFiled` the standing UnfiledCopy question already says it: with a
                    // button, which a transient hint has nowhere to put.
                    SendStatus::Idle | SendStatus::SentNotFiled => None,
                };
            }
            // The question raised when a calendar server stored the answer and then reported it
            // could not pass it on. The core holds it and clears it; this only mirrors.
            Surface::InvitationReply => {
                self.reply_prompt = app.reply_prompt();
                self.reply_prompt_generation = self.reply_prompt_generation.wrapping_add(1);
            }
            // A message the core withdrew from the Outbox so the user could change it. It
            // exists nowhere else by the time this arrives, so this may not refuse.
            Surface::ComposeRequest => {
                if let Some(request) = app.compose_request() {
                    self.open_withdrawn_message(request);
                }
            }
            Surface::UnfiledCopy => {
                self.unfiled_copy = app.unfiled_copy().map(|copy| UnfiledCopyNotice {
                    body: l10n::unfiled_copy_body(&copy.subject),
                    retrying: copy.retrying,
                });
            }
            // Both progress surfaces share one snapshot and one strip. The foreground bar wins
            // while active; the hint is the quiet caption for a pass nobody started.
            Surface::SyncProgress => {
                let progress = app.sync_progress();
                self.sync_bar = model::sync_bar(&progress);
                self.sync_hint = model::sync_hint(&progress, &self.snapshot.accounts);
            }
            Surface::Calendar => self.calendar.refresh(&app),
            Surface::Contacts => self.contacts.refresh(&app),
            Surface::ContactsStatus => self.contacts.refresh_write_status(&app),
            Surface::CalendarStatus => self.calendar.refresh_write_status(&app),
            Surface::Settings => self.calendar.refresh_settings(&app),
            Surface::Connectivity => {
                self.connectivity = ConnectivityState::pull(&app, &self.snapshot.accounts);
            }
        }
    }
}
