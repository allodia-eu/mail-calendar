//! What the account pages have running, and what each change came to (`docs/waiting.md`).
//!
//! One change at a time across every account: a change to one account can write another's config
//! (a link is stored on both ends), so while one runs every account page holds its controls, and
//! only the page that started it marks its control running. A change that still arrives while
//! another runs (a field sending what was typed as the held page takes its focus) waits and runs
//! next; the same work asked for again is dropped. Kept here rather than by any client, so a page
//! drawn on any platform says the same thing about the same work.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{
    AccountCapability, AccountChangeNotice, AccountChangeNoticeRaised, AccountPageIntent, LinkSlot,
};

/// A change an account's page started, with where it takes the control, so a running control is
/// drawn at the value it is moving to rather than back at the one it is leaving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PageWork {
    Use {
        capability: AccountCapability,
        on: bool,
    },
    Link {
        slot: LinkSlot,
        target: Option<String>,
    },
    ReplyFallback {
        choice: crate::ReplyFallbackChoice,
    },
    Remove,
    SenderName(String),
    Depth(u16),
    Size(u16),
    Push(bool),
    Interval(u16),
    Folder {
        key: String,
        on: bool,
    },
    Servers(Box<crate::AccountEndpoints>),
}

impl PageWork {
    /// The work `intent` starts, with where it takes its control.
    pub(crate) fn of(intent: &AccountPageIntent) -> Self {
        match intent {
            AccountPageIntent::SetUse { capability, on, .. } => Self::Use {
                capability: *capability,
                on: *on,
            },
            AccountPageIntent::SetLink { slot, target, .. } => Self::Link {
                slot: *slot,
                target: target.clone(),
            },
            AccountPageIntent::SetReplyFallback { choice, .. } => {
                Self::ReplyFallback { choice: *choice }
            }
            AccountPageIntent::Remove { .. } => Self::Remove,
            AccountPageIntent::SetSenderName { name, .. } => Self::SenderName(name.clone()),
            AccountPageIntent::SetSyncDepth { months, .. } => Self::Depth(*months),
            AccountPageIntent::SetMessageSizeLimit { megabytes, .. } => Self::Size(*megabytes),
            AccountPageIntent::SetPush { on, .. } => Self::Push(*on),
            AccountPageIntent::SetPollInterval { minutes, .. } => Self::Interval(*minutes),
            AccountPageIntent::SetWatchedFolder { key, on, .. } => Self::Folder {
                key: key.clone(),
                on: *on,
            },
            AccountPageIntent::SaveServers { endpoints, .. } => {
                Self::Servers(Box::new(endpoints.clone()))
            }
        }
    }
}

/// How a page's controls stand against the change running, if any.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Hold {
    /// A change runs somewhere, so every control is held.
    pub(crate) busy: bool,
    /// The changes this page started, running or waiting in the order they run, whose controls
    /// are drawn running.
    pub(crate) own: Vec<PageWork>,
}

impl Hold {
    /// Where this page's work takes the control `at` picks out, from the last change that moves
    /// it, which is where it ends up; `None` when none of it does.
    pub(crate) fn going<'a, T>(&'a self, at: impl Fn(&'a PageWork) -> Option<T>) -> Option<T> {
        self.own.iter().rev().find_map(at)
    }
}

/// What asking for a change came to.
#[derive(Debug)]
pub(crate) enum Start {
    /// Nothing else runs: run it now.
    Run(AccountPageIntent),
    /// It runs once the changes ahead of it end.
    Waits,
    /// The same work is already running or waiting.
    Dropped,
}

/// The change running and those waiting behind it, each with the account it is for.
#[derive(Debug, Default)]
struct Queue {
    running: Option<(String, PageWork)>,
    waiting: VecDeque<(String, PageWork, AccountPageIntent)>,
}

/// Every account page's state.
#[derive(Debug, Default)]
pub(crate) struct AccountWork {
    queue: Mutex<Queue>,
    notices: Mutex<HashMap<String, AccountChangeNoticeRaised>>,
    /// What a removal came to: said on the accounts list, since the page is gone with the account.
    list_notice: Mutex<Option<AccountChangeNoticeRaised>>,
    /// The edit of each account's servers its last save refused, which its form is drawn with.
    refused_servers: Mutex<HashMap<String, crate::AccountEndpoints>>,
    raised: AtomicU64,
}

impl AccountWork {
    /// Asks for `intent`'s change: it runs now when nothing else does, and takes away the notice
    /// the last change on its page left; otherwise it waits its turn, unless the same work is
    /// already running or waiting.
    pub(crate) fn start(&self, intent: AccountPageIntent) -> Start {
        let (account, work) = (intent.account().to_owned(), PageWork::of(&intent));
        let mut queue = self.queue.lock().expect("account work mutex poisoned");
        let same = |id: &String, other: &PageWork| *id == account && *other == work;
        if queue
            .running
            .as_ref()
            .is_some_and(|(id, other)| same(id, other))
            || queue.waiting.iter().any(|(id, other, _)| same(id, other))
        {
            return Start::Dropped;
        }
        if queue.running.is_some() {
            queue.waiting.push_back((account, work, intent));
            return Start::Waits;
        }
        queue.running = Some((account.clone(), work));
        drop(queue);
        // Only the notice: what the page is drawn from outlives a change.
        self.notices
            .lock()
            .expect("account notice mutex poisoned")
            .remove(&account);
        Start::Run(intent)
    }

    /// The change for `account` ended, saying `notice` when it needs saying: on the account's
    /// page, or on the accounts list when the change removed it. Answers the change that runs
    /// next, now marked running; a waiting change for an account the ended one removed is dropped.
    /// The next change leaves the notice standing: it was asked for before the notice was raised.
    pub(crate) fn finish(
        &self,
        account: &str,
        notice: Option<AccountChangeNotice>,
    ) -> Option<AccountPageIntent> {
        let mut queue = self.queue.lock().expect("account work mutex poisoned");
        let removed = matches!(queue.running, Some((_, PageWork::Remove)));
        let raised = notice.map(|notice| AccountChangeNoticeRaised {
            id: self.raised.fetch_add(1, Ordering::Relaxed) + 1,
            notice,
        });
        if removed {
            queue.waiting.retain(|(id, ..)| id != account);
            self.forget(account);
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
        let Some((next_account, work, intent)) = queue.waiting.pop_front() else {
            queue.running = None;
            return None;
        };
        queue.running = Some((next_account, work));
        Some(intent)
    }

    /// Drops what `account`'s page had to say, for an account that is gone: one added again later
    /// opens a page of its own.
    pub(crate) fn forget(&self, account: &str) {
        self.notices
            .lock()
            .expect("account notice mutex poisoned")
            .remove(account);
        self.servers_saved(account, None);
    }

    /// What `account`'s last save of its servers came to: `refused` is the edit it did not keep,
    /// `None` once one was kept.
    pub(crate) fn servers_saved(&self, account: &str, refused: Option<crate::AccountEndpoints>) {
        let mut kept = self
            .refused_servers
            .lock()
            .expect("account work mutex poisoned");
        match refused {
            Some(edit) => kept.insert(account.to_owned(), edit),
            None => kept.remove(account),
        };
    }

    /// The edit of `account`'s servers its last save refused.
    pub(crate) fn refused_servers(&self, account: &str) -> Option<crate::AccountEndpoints> {
        self.refused_servers
            .lock()
            .expect("account work mutex poisoned")
            .get(account)
            .cloned()
    }

    /// How `account`'s page stands, and the notice it shows.
    pub(crate) fn of(&self, account: &str) -> (Hold, Option<AccountChangeNoticeRaised>) {
        let queue = self.queue.lock().expect("account work mutex poisoned");
        let hold = Hold {
            busy: queue.running.is_some(),
            own: queue
                .running
                .iter()
                .map(|(id, work)| (id, work))
                .chain(queue.waiting.iter().map(|(id, work, _)| (id, work)))
                .filter(|(id, _)| *id == account)
                .map(|(_, work)| work.clone())
                .collect(),
        };
        drop(queue);
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

#[cfg(test)]
#[path = "account_work_tests.rs"]
mod tests;
