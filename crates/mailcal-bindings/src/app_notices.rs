//! The notices on [`MailcalApp`]: the snapshot a host pulls after `Surface::Notices`, and what
//! dismissing one or reporting a failed remedy does (`docs/notices.md`).

use std::sync::Arc;

use crate::{
    FolderNotice, Intent, MailcalApp, NoticeIntent, NoticeScreens, NoticesSnapshot,
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
        let addresses = self.runtime.block_on(|| self.app.account_addresses());
        // An account gone between the two reads is named by its id rather than not at all.
        let address = |account: &str| {
            addresses
                .get(account)
                .cloned()
                .unwrap_or_else(|| account.to_owned())
        };
        let questions = self.questions();
        let signing_in = self.account_work.browser.account();
        let facts = NoticeFacts {
            connectivity: &connectivity,
            skipped_at_launch,
            folder_notice: self.app.folder_notice().map(Into::into),
            provider: &provider,
            address: &address,
            questions: &questions,
            signing_in: signing_in.as_deref(),
        };
        let raised = raise(&facts);
        // A pause ending, or a lookup replacing the question, reaches every client as this
        // signal, so this is where a failed allow it made stale is let go.
        self.account_work
            .keep_server_change_failures(|account, failed| {
                engine_api::AccountId::try_from(account)
                    .ok()
                    .and_then(|id| self.app.server_change(&id))
                    .is_some_and(|waiting| crate::ServerChange::from(&waiting) == *failed)
            });
        self.notice_book
            .lock()
            .expect("notice-book mutex poisoned")
            .snapshot(raised)
    }
}

impl MailcalApp {
    /// The questions the core is holding, for the notices that ask them.
    fn questions(&self) -> crate::notices::Questions {
        use crate::notices::{Questions, ReplyQuestion, UnfiledQuestion};
        Questions {
            unfiled: self.app.unfiled_copy().map(|unfiled| UnfiledQuestion {
                account: unfiled.account.as_str().to_owned(),
                subject: unfiled.subject,
                retrying: unfiled.retrying,
            }),
            reply: self.app.reply_prompt().map(|prompt| ReplyQuestion {
                account: prompt.account.as_str().to_owned(),
                summary: prompt.summary,
                organizer: prompt.organizer,
                response: prompt.response.into(),
            }),
            time_zone: self
                .app
                .timezone_settings()
                .pending_device
                .map(|zone| zone.as_str().to_owned()),
        }
    }

    /// Handles an `Intent::Notices`, on the host's thread like the other intents the binding layer
    /// answers itself.
    pub(crate) fn dispatch_notice(&self, intent: NoticeIntent) {
        // Read first, so the book judges the id against what is in force now.
        let shown = self.notices();
        let mut book = self.notice_book.lock().expect("notice-book mutex poisoned");
        match intent {
            NoticeIntent::Dismiss { id } => match book.dismiss(&id) {
                Some(Dismissal::Remembered) => {
                    drop(book);
                    self.app.signal_notices();
                }
                // The pane holds it, and clearing it there signals the notices. Only while it is
                // still this refusal: a later one may have replaced it since.
                Some(Dismissal::FolderNotice(shown)) => {
                    drop(book);
                    let app = Arc::clone(&self.app);
                    self.runtime.spawn(async move {
                        app.dismiss_folder_notice_if(|held| {
                            FolderNotice::from(held.clone()) == shown
                        })
                        .await;
                    });
                }
                None => {}
            },
            NoticeIntent::RemedyFailed { id } => {
                if book.note_remedy_failed(&id) {
                    drop(book);
                    self.app.signal_notices();
                }
            }
            NoticeIntent::Retry { id } => {
                if !book.note_retry(&id) {
                    return;
                }
                drop(book);
                let screens = shown.notices.iter().find(|n| n.id == id).map(|n| n.screens);
                // The refresh of the notice's screen is what dials the account again; the dial
                // settles the retry when it ends (`settle_notice_retries`).
                self.dispatch(match screens {
                    Some(NoticeScreens::Calendar) => Intent::RefreshCalendar,
                    Some(NoticeScreens::Contacts) => Intent::RefreshContacts,
                    _ => Intent::RefreshMail,
                });
            }
        }
    }

    /// A re-dial has ended: every retried notice still in force did not get through, and says so.
    pub(crate) fn settle_notice_retries(&self) {
        let _ = self.notices();
        let marked = self
            .notice_book
            .lock()
            .expect("notice-book mutex poisoned")
            .settle_retries();
        if !marked.is_empty() {
            log::info!("notices: {} retry(ies) did not get through", marked.len());
            self.app.signal_notices();
        }
    }
}

#[cfg(test)]
#[path = "app_notices_tests.rs"]
mod tests;
