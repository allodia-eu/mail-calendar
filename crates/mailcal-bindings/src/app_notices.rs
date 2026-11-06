//! The notices on [`MailcalApp`]: the snapshot a host pulls after `Surface::Notices`, and what
//! dismissing one or reporting a failed remedy does (`docs/notices.md`).

use std::sync::Arc;

use mailcal_app::{FolderIntent as AppFolderIntent, Intent as AppIntent};

use crate::{
    MailcalApp, NoticeIntent, NoticesSnapshot,
    notices::{Dismissal, NoticeFacts, raise},
};

#[uniffi::export]
impl MailcalApp {
    /// Every notice in force, ranked (pulled after a `Surface::Notices` signal).
    pub fn notices(&self) -> NoticesSnapshot {
        let connectivity = self.connectivity();
        let skipped_at_launch = self
            .account_connect_errors
            .lock()
            .expect("account-errors mutex poisoned")
            .len();
        let provider = |account: &str| self.registry.provider(account);
        let facts = NoticeFacts {
            connectivity: &connectivity,
            skipped_at_launch,
            folder_notice: self.app.folder_notice().map(Into::into),
            provider: &provider,
        };
        let raised = raise(&facts);
        self.notice_book
            .lock()
            .expect("notice-book mutex poisoned")
            .snapshot(raised)
    }
}

impl MailcalApp {
    /// Handles an `Intent::Notices`, on the host's thread like the other intents the binding layer
    /// answers itself.
    pub(crate) fn dispatch_notice(&self, intent: NoticeIntent) {
        // Read first, so the book judges the id against what is in force now.
        let _ = self.notices();
        let mut book = self.notice_book.lock().expect("notice-book mutex poisoned");
        match intent {
            NoticeIntent::Dismiss { id } => match book.dismiss(&id) {
                Some(Dismissal::Remembered) => {
                    drop(book);
                    self.app.signal_notices();
                }
                // The pane holds it, and clearing it there signals the notices.
                Some(Dismissal::FolderNotice) => {
                    drop(book);
                    let app = Arc::clone(&self.app);
                    let intent = AppIntent::Folders(AppFolderIntent::DismissNotice);
                    self.runtime
                        .spawn(async move { app.dispatch(intent).await });
                }
                None => {}
            },
            NoticeIntent::RemedyFailed { id } => {
                if book.note_remedy_failed(&id) {
                    drop(book);
                    self.app.signal_notices();
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "app_notices_tests.rs"]
mod tests;
