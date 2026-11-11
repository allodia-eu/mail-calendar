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
    /// Stopping the links between the account and `from`.
    Unlink {
        from: String,
    },
    ReplyFallback {
        choice: crate::ReplyFallbackChoice,
    },
    Sharing(crate::AllodiaAccountSyncMode),
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
    Shared {
        handle: String,
        on: bool,
    },
    AddShared(String),
    /// A typed password, never kept: the same work asked twice is one press, whatever was typed.
    Password,
    /// Allowing the new servers the question showed.
    ServerChange(crate::ServerChange),
    /// Signing the separate server in as this login; the password is never kept.
    SeparateServer {
        username: String,
    },
    /// Switching off the uses the separate server holds.
    StopSeparate,
    /// Completing the browser sign-in `attempt`.
    SignIn {
        attempt: u64,
    },
}

/// A change the page queue runs: one an account's page asked for, or the completion of a browser
/// sign-in, which waits its turn like any change since it writes the account too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Change {
    Page(AccountPageIntent),
    SignIn { account: String, attempt: u64 },
}

impl From<AccountPageIntent> for Change {
    fn from(intent: AccountPageIntent) -> Self {
        Self::Page(intent)
    }
}

impl Change {
    /// The account the change is to.
    pub(crate) fn account(&self) -> &str {
        match self {
            Self::Page(intent) => intent.account(),
            Self::SignIn { account, .. } => account,
        }
    }

    fn work(&self) -> PageWork {
        match self {
            Self::Page(intent) => PageWork::of(intent),
            Self::SignIn { attempt, .. } => PageWork::SignIn { attempt: *attempt },
        }
    }
}

impl PageWork {
    /// The work `intent` starts, with where it takes its control.
    pub(crate) fn of(intent: &AccountPageIntent) -> Self {
        match intent {
            // Never queued: each is a step of the browser sign-in, which queues only its
            // completion.
            AccountPageIntent::SignIn { .. }
            | AccountPageIntent::SignInSeparateServer { .. }
            | AccountPageIntent::AnswerUnreached { .. }
            | AccountPageIntent::SignInBrowserEnded { .. }
            | AccountPageIntent::CancelSignIn { .. } => Self::SignIn { attempt: 0 },
            AccountPageIntent::SetUse { capability, on, .. } => Self::Use {
                capability: *capability,
                on: *on,
            },
            AccountPageIntent::SetLink { slot, target, .. } => Self::Link {
                slot: *slot,
                target: target.clone(),
            },
            AccountPageIntent::StopLinking { from, .. } => Self::Unlink { from: from.clone() },
            AccountPageIntent::SetReplyFallback { choice, .. } => {
                Self::ReplyFallback { choice: *choice }
            }
            AccountPageIntent::SetSharing { mode, .. } => Self::Sharing(*mode),
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
            AccountPageIntent::SetSharedMailbox { handle, on, .. } => Self::Shared {
                handle: handle.clone(),
                on: *on,
            },
            AccountPageIntent::AddSharedMailbox { address, .. } => Self::AddShared(address.clone()),
            AccountPageIntent::UsePassword { .. } => Self::Password,
            AccountPageIntent::AllowServerChange { shown, .. } => Self::ServerChange(shown.clone()),
            AccountPageIntent::ConnectSeparateServer { username, .. } => Self::SeparateServer {
                username: username.clone(),
            },
            AccountPageIntent::StopUsingSeparateServer { .. } => Self::StopSeparate,
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
    Run(Change),
    /// It runs once the changes ahead of it end.
    Waits,
    /// The same work is already running or waiting.
    Dropped,
}

/// The change running and those waiting behind it, each with the account it is for.
#[derive(Debug, Default)]
struct Queue {
    running: Option<(String, PageWork)>,
    waiting: VecDeque<(String, PageWork, Change)>,
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
    /// What each account's server last said of the mailboxes its sign-in opens.
    pub(crate) shared: crate::account_page::shared::SharedAnswers,
    /// What each account's server last said of the ways in it takes.
    pub(crate) sign_in: crate::account_page::sign_in::SignInAnswers,
    /// The server change each account's last allow did not move it onto.
    server_change_failed: Mutex<HashMap<String, crate::ServerChange>>,
    /// What each account's separate server last said of the ways in it takes besides a password,
    /// and what its last Connect or Skip came to.
    pub(crate) separate: crate::account_page::separate::SeparateState,
    /// The browser sign-in out, and the Nextcloud grant kept from the last.
    pub(crate) browser: crate::account_page::browser::BrowserSignIns,
    raised: AtomicU64,
}

impl AccountWork {
    /// Asks for `intent`'s change: it runs now when nothing else does, and takes away the notice
    /// the last change on its page left; otherwise it waits its turn, unless the same work is
    /// already running or waiting.
    pub(crate) fn start(&self, change: impl Into<Change>) -> Start {
        let intent = change.into();
        let (account, work) = (intent.account().to_owned(), intent.work());
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
        self.unsay(&account);
        Start::Run(intent)
    }

    /// Takes away what `account`'s page last said, because its next change has started: one
    /// queued here, or a browser sign-in, which reaches the queue only once it completes.
    pub(crate) fn unsay(&self, account: &str) {
        self.notices
            .lock()
            .expect("account notice mutex poisoned")
            .remove(account);
    }

    /// The change for `account` ended, saying `notice` when it needs saying: on the account's
    /// page, or on the accounts list when the change removed it. Answers the change that runs
    /// next, now marked running; a waiting change for an account the ended one removed is dropped.
    /// The next change leaves the notice standing: it was asked for before the notice was raised.
    pub(crate) fn finish(
        &self,
        account: &str,
        notice: Option<AccountChangeNotice>,
    ) -> Option<Change> {
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

    /// Raises `notice` on `account`'s page outside a change: what a browser sign-in came to before
    /// it reached its completion.
    pub(crate) fn say(&self, account: &str, notice: AccountChangeNotice) {
        let raised = AccountChangeNoticeRaised {
            id: self.raised.fetch_add(1, Ordering::Relaxed) + 1,
            notice,
        };
        self.notices
            .lock()
            .expect("account notice mutex poisoned")
            .insert(account.to_owned(), raised);
    }

    /// Drops what `account`'s page had to say, for an account that is gone: one added again later
    /// opens a page of its own.
    pub(crate) fn forget(&self, account: &str) {
        self.notices
            .lock()
            .expect("account notice mutex poisoned")
            .remove(account);
        self.servers_saved(account, None);
        self.shared.forget(account);
        self.sign_in.forget(account);
        self.server_change_tried(account, None);
        self.separate.forget(account);
    }

    /// What `account`'s last allow of a server change came to: `failed` is the change it did not
    /// move the account onto, `None` once one settled.
    pub(crate) fn server_change_tried(&self, account: &str, failed: Option<crate::ServerChange>) {
        let mut kept = self
            .server_change_failed
            .lock()
            .expect("account work mutex poisoned");
        match failed {
            Some(change) => kept.insert(account.to_owned(), change),
            None => kept.remove(account),
        };
    }

    /// Keeps a failed allow only while `waits` says its account still waits on that very change:
    /// a pause that ended, or a question a lookup replaced, leaves nothing for a later one to
    /// inherit, though the lookup may propose the same servers again.
    pub(crate) fn keep_server_change_failures(
        &self,
        waits: impl Fn(&str, &crate::ServerChange) -> bool,
    ) {
        self.server_change_failed
            .lock()
            .expect("account work mutex poisoned")
            .retain(|account, failed| waits(account, failed));
    }

    /// Whether `account`'s last allow was of `change`, and did not move it.
    pub(crate) fn server_change_failed(&self, account: &str, change: &crate::ServerChange) -> bool {
        self.server_change_failed
            .lock()
            .expect("account work mutex poisoned")
            .get(account)
            == Some(change)
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
