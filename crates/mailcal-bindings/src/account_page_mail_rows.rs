//! The row each account's mail half is drawn from, kept so an account's page is read from memory
//! (`docs/waiting.md` rule 4). The row reads the store and the live account list, so it is read
//! off the thread that draws the page: in the background whenever the page or the accounts list is
//! read, and on a change's own thread as the change starts and before it ends.

use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use mailcal_viewmodel::{AccountSyncRow, SyncStrategyKind};

use crate::account_work::{Hold, PageWork};

#[derive(Debug, Default)]
struct Kept {
    /// The row last read: `None` for an account not used for mail. Absent before the first read.
    row: Option<Option<AccountSyncRow>>,
    /// The read the kept row came from, so an older read landing later is dropped.
    read: u64,
    /// A read is out in the background.
    asking: bool,
    /// The row was asked for again while that read was out, so it reads once more when it lands.
    again: bool,
}

/// Every account's row.
#[derive(Debug, Default)]
pub(crate) struct MailRows {
    kept: Mutex<HashMap<String, Kept>>,
    reads: AtomicU64,
}

impl MailRows {
    fn kept(&self) -> std::sync::MutexGuard<'_, HashMap<String, Kept>> {
        self.kept.lock().expect("mail rows mutex poisoned")
    }

    /// `account`'s row as last read; `None` before it was read, and for an account not used for
    /// mail.
    pub(crate) fn row(&self, account: &str) -> Option<AccountSyncRow> {
        self.kept()
            .get(account)
            .and_then(|kept| kept.row.clone().flatten())
    }

    /// Numbers a read about to start, taken before anything it reads.
    pub(crate) fn begin(&self) -> u64 {
        self.reads.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// The read `read` found `row`. Answers whether the page draws something else now: an older
    /// read than the one kept changes nothing.
    pub(crate) fn found(&self, account: &str, read: u64, row: Option<AccountSyncRow>) -> bool {
        let mut kept = self.kept();
        let kept = kept.entry(account.to_owned()).or_default();
        if read < kept.read {
            return false;
        }
        kept.read = read;
        let before = kept.row.replace(row);
        before.flatten() != kept.row.clone().flatten()
    }

    /// Whether a read in the background should start for `account`: not while one is out, which
    /// then reads once more when it lands.
    pub(crate) fn ask(&self, account: &str) -> bool {
        let mut kept = self.kept();
        let kept = kept.entry(account.to_owned()).or_default();
        if kept.asking {
            kept.again = true;
            return false;
        }
        kept.asking = true;
        true
    }

    /// The background read for `account` landed. Answers whether it reads once more, since it was
    /// asked for again meanwhile; otherwise the next ask starts one.
    pub(crate) fn landed(&self, account: &str) -> bool {
        let mut kept = self.kept();
        let Some(kept) = kept.get_mut(account) else {
            return false;
        };
        if kept.again {
            kept.again = false;
            return true;
        }
        kept.asking = false;
        false
    }

    /// Drops what is kept for `account`, which is gone; a read still out for it keeps nothing.
    pub(crate) fn forget(&self, account: &str) {
        let read = self.begin();
        let mut kept = self.kept();
        kept.remove(account);
        // Marked read now, so a read begun before it was removed does not bring it back.
        kept.insert(
            account.to_owned(),
            Kept {
                read,
                ..Kept::default()
            },
        );
    }
}

/// The strategy a running switch moves the account to: its row is drawn as the switch will store
/// it, its watched folders included.
pub(crate) fn going_strategy(hold: &Hold) -> Option<SyncStrategyKind> {
    hold.going(|work| match work {
        PageWork::Push(on) => Some(if *on {
            SyncStrategyKind::Push
        } else {
            SyncStrategyKind::Poll
        }),
        _ => None,
    })
}

#[cfg(test)]
#[path = "account_page_mail_rows_tests.rs"]
mod tests;
