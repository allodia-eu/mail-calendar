//! Sent mail older than the account's sync window, for a learning review (`docs/ai.md` rule 11):
//! the account's Sent mailboxes are held further back while the review fetches and reads them,
//! then the account is narrowed back to its own window as a sync depth reduction narrows it, so
//! the device keeps what the person asked it to keep.
//!
//! While they are held further back, every pass of the account syncs under the deeper windows,
//! so a poll that runs meanwhile neither fights the fetch nor reconciles the older mail away.

use std::collections::HashMap;

use engine_api::{AccountId, CalendarDate, MailboxId, MailboxWindows, Provider, SyncWindow};
use time::Date;

use crate::{
    App,
    mail_ops::sent_mailboxes,
    sync::{ACCOUNT_SYNC_BUSY_RETRY_ATTEMPTS, busy_retry_delay, sync_window},
};

/// The Sent mailboxes held further back while a review reads them, by account, with the window
/// they are held to.
pub(crate) type HeldDeeper = HashMap<AccountId, (Vec<MailboxId>, SyncWindow)>;

/// The window a range whose first day is `first_day` needs: from that day, or the whole history.
fn window_from(first_day: Option<Date>) -> SyncWindow {
    first_day
        .and_then(|day| CalendarDate::new(day.year(), u8::from(day.month()), day.day()).ok())
        .map_or_else(SyncWindow::full, SyncWindow::since)
}

impl<P: Provider> App<P> {
    pub(super) fn held_deeper(&self) -> std::sync::MutexGuard<'_, HeldDeeper> {
        self.ai
            .writing_style
            .deeper
            .lock()
            .expect("held deeper mutex poisoned")
    }

    /// The windows `id`'s mail syncs under: its sync depth, with its Sent mailboxes further back
    /// while a learning review reads them.
    pub(crate) fn mailbox_windows_for(&self, id: &AccountId) -> MailboxWindows {
        let windows = MailboxWindows::new(sync_window(self.effective_sync_depth(id.as_str())));
        match self.held_deeper().get(id) {
            Some((mailboxes, window)) => mailboxes.iter().fold(windows, |windows, mailbox| {
                windows.deepen(mailbox.clone(), *window)
            }),
            None => windows,
        }
    }

    /// Holds `account`'s Sent mailboxes back to `first_day` (`None`: their whole history) and
    /// syncs the account under that, waiting out a pass another sync holds. Whether the pass
    /// ended with nothing failed; one that did not may still have fetched what a range needs,
    /// which the read tells from how far back the device then holds the Sent mail
    /// (`MailboxSpan::oldest`). Held until [`App::release_older_sent`], which every caller runs,
    /// whatever this answered.
    pub(super) async fn fetch_older_sent(
        &self,
        account: &AccountId,
        first_day: Option<Date>,
    ) -> bool {
        let mailboxes = self.engine.mailboxes(account).await.unwrap_or_default();
        let sent: Vec<MailboxId> = sent_mailboxes(&mailboxes)
            .map(|mailbox| mailbox.id.clone())
            .collect();
        if sent.is_empty() {
            return false;
        }
        self.held_deeper()
            .insert(account.clone(), (sent, window_from(first_day)));
        let acct = self.account_ordinal(account).await;
        log::info!("learning[a{acct}]: fetching older sent mail");
        let mut complete = false;
        for attempt in 0..ACCOUNT_SYNC_BUSY_RETRY_ATTEMPTS {
            let Some(outcome) = self.refresh_account_once(account, "learning", true).await else {
                break;
            };
            if outcome.busy_scopes == 0 {
                complete = outcome.complete;
                break;
            }
            tokio::time::sleep(busy_retry_delay(attempt)).await;
        }
        if !complete {
            log::info!("learning[a{acct}]: the pass fetching older sent mail did not complete");
        }
        complete
    }

    /// Narrows `account` back to its own window after a review read its Sent mail further back:
    /// what was fetched further back is removed from the device, and since the cursors still
    /// record the deeper fetch, they are cleared and the account synced again under its own
    /// window, as a sync depth reduction is. A no-op when nothing was held further back.
    pub(super) async fn release_older_sent(&self, account: &AccountId) {
        if self.held_deeper().remove(account).is_none() {
            return;
        }
        let acct = self.account_ordinal(account).await;
        let windows = self.mailbox_windows_for(account);
        match self
            .engine
            .prune_account_mail_outside_window(account, windows)
            .await
        {
            Ok(report) => log::info!(
                "learning[a{acct}]: removed {} older sent message(s) after the read",
                report.messages_removed,
            ),
            Err(err) => log::warn!("learning[a{acct}]: removing older sent mail failed: {err}"),
        }
        self.invalidate_list_cache();
        self.rebuild_snapshot().await;
        self.resync_account_after_depth_change(account).await;
        self.reclaim_freed_space("learning").await;
    }
}
