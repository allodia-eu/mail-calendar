//! What the account pages have running, and what each change came to (`docs/waiting.md`).
//!
//! One change at a time across every account: a change to one account can write another's config
//! (a link is stored on both ends), so while one runs every account page holds its controls, and
//! only the page that started it marks its control running. Kept here rather than by any client,
//! so a page drawn on any platform says the same thing about the same work.

use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{AccountCapability, AccountChangeNotice, AccountChangeNoticeRaised, LinkSlot};

/// A change an account's page started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageWork {
    Use(AccountCapability),
    Link(LinkSlot),
    Remove,
}

/// How a page's controls stand against the change running, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Hold {
    /// A change runs somewhere, so every control is held.
    pub(crate) busy: bool,
    /// The change this page started, whose control is drawn running.
    pub(crate) own: Option<PageWork>,
}

/// Every account page's state.
#[derive(Debug, Default)]
pub(crate) struct AccountWork {
    running: Mutex<Option<(String, PageWork)>>,
    notices: Mutex<HashMap<String, AccountChangeNoticeRaised>>,
    /// What a removal came to: said on the accounts list, since the page is gone with the account.
    list_notice: Mutex<Option<AccountChangeNoticeRaised>>,
    raised: AtomicU64,
}

impl AccountWork {
    /// Marks `work` running for `account`, and takes away the notice the last change on its page
    /// left. `false` while another change runs, and then nothing starts.
    pub(crate) fn start(&self, account: &str, work: PageWork) -> bool {
        let mut running = self.running.lock().expect("account work mutex poisoned");
        if running.is_some() {
            return false;
        }
        *running = Some((account.to_owned(), work));
        self.notices
            .lock()
            .expect("account notice mutex poisoned")
            .remove(account);
        true
    }

    /// The change for `account` ended, saying `notice` when it needs saying: on the account's
    /// page, or on the accounts list when the change removed it.
    pub(crate) fn finish(&self, account: &str, notice: Option<AccountChangeNotice>) {
        let removed = matches!(
            *self.running.lock().expect("account work mutex poisoned"),
            Some((_, PageWork::Remove))
        );
        let raised = notice.map(|notice| AccountChangeNoticeRaised {
            id: self.raised.fetch_add(1, Ordering::Relaxed) + 1,
            notice,
        });
        if removed {
            self.notices
                .lock()
                .expect("account notice mutex poisoned")
                .remove(account);
            *self
                .list_notice
                .lock()
                .expect("account notice mutex poisoned") = raised;
        } else if let Some(raised) = raised {
            self.notices
                .lock()
                .expect("account notice mutex poisoned")
                .insert(account.to_owned(), raised);
        }
        *self.running.lock().expect("account work mutex poisoned") = None;
    }

    /// Drops what `account`'s page had to say, for an account that is gone: one added again later
    /// opens a page of its own.
    pub(crate) fn forget(&self, account: &str) {
        self.notices
            .lock()
            .expect("account notice mutex poisoned")
            .remove(account);
    }

    /// How `account`'s page stands, and the notice it shows.
    pub(crate) fn of(&self, account: &str) -> (Hold, Option<AccountChangeNoticeRaised>) {
        let running = self.running.lock().expect("account work mutex poisoned");
        let hold = Hold {
            busy: running.is_some(),
            own: running
                .as_ref()
                .filter(|(id, _)| id == account)
                .map(|(_, work)| *work),
        };
        drop(running);
        let notice = self
            .notices
            .lock()
            .expect("account notice mutex poisoned")
            .get(account)
            .cloned();
        (hold, notice)
    }

    /// What the last removal came to, when it needs saying.
    pub(crate) fn list_notice(&self) -> Option<AccountChangeNoticeRaised> {
        self.list_notice
            .lock()
            .expect("account notice mutex poisoned")
            .clone()
    }
}
