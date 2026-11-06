//! What an account's page has running, and what its last change came to (`docs/waiting.md`).
//!
//! One piece of work per account at a time: while one runs every control on its page is held, so
//! two changes to one account never race. Kept here rather than by any client, so a page drawn on
//! any platform says the same thing about the same work.

use std::{collections::HashMap, sync::Mutex};

use crate::{AccountCapability, AccountChangeNotice, AccountChangeNoticeRaised, LinkSlot};

/// A change an account's page started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageWork {
    Use(AccountCapability),
    Link(LinkSlot),
    Remove,
}

#[derive(Debug, Default)]
struct PageState {
    running: Option<PageWork>,
    notice: Option<AccountChangeNoticeRaised>,
}

/// Every account's page state.
#[derive(Debug, Default)]
pub(crate) struct AccountWork {
    pages: Mutex<HashMap<String, PageState>>,
    notices: Mutex<u64>,
}

impl AccountWork {
    /// Marks `work` running on `account`'s page, and takes away the notice the last change left.
    /// `false` while other work runs there, and then nothing starts.
    pub(crate) fn start(&self, account: &str, work: PageWork) -> bool {
        let mut pages = self.pages.lock().expect("account work mutex poisoned");
        let page = pages.entry(account.to_owned()).or_default();
        if page.running.is_some() {
            return false;
        }
        page.running = Some(work);
        page.notice = None;
        true
    }

    /// The work on `account`'s page ended, saying `notice` when it needs saying.
    pub(crate) fn finish(&self, account: &str, notice: Option<AccountChangeNotice>) {
        let raised = notice.map(|notice| {
            let mut id = self.notices.lock().expect("account notice mutex poisoned");
            *id += 1;
            AccountChangeNoticeRaised { id: *id, notice }
        });
        let mut pages = self.pages.lock().expect("account work mutex poisoned");
        let page = pages.entry(account.to_owned()).or_default();
        page.running = None;
        page.notice = raised;
    }

    /// Forgets a removed account's page.
    pub(crate) fn forget(&self, account: &str) {
        self.pages
            .lock()
            .expect("account work mutex poisoned")
            .remove(account);
    }

    /// What runs on `account`'s page, and the notice it shows.
    pub(crate) fn of(
        &self,
        account: &str,
    ) -> (Option<PageWork>, Option<AccountChangeNoticeRaised>) {
        self.pages
            .lock()
            .expect("account work mutex poisoned")
            .get(account)
            .map_or((None, None), |page| (page.running, page.notice.clone()))
    }
}
