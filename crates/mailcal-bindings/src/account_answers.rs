//! What an account's server last said about something its page draws: asked in the background,
//! never on the thread that reads the page, and kept so the page is drawn on every signal without
//! asking again.

use std::{
    collections::HashMap,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Debug)]
struct Answer<T> {
    value: Option<T>,
    /// The ask out now, if any: one at a time.
    asking: Option<u64>,
}

impl<T> Default for Answer<T> {
    fn default() -> Self {
        Self {
            value: None,
            asking: None,
        }
    }
}

/// What each account's server last answered.
#[derive(Debug)]
pub(crate) struct Answers<T> {
    answers: Mutex<HashMap<String, Answer<T>>>,
    tickets: AtomicU64,
}

impl<T> Default for Answers<T> {
    fn default() -> Self {
        Self {
            answers: Mutex::default(),
            tickets: AtomicU64::default(),
        }
    }
}

impl<T: Clone + PartialEq> Answers<T> {
    fn answers(&self) -> MutexGuard<'_, HashMap<String, Answer<T>>> {
        self.answers.lock().expect("server answers mutex poisoned")
    }

    /// A ticket to ask `account`'s server, when no ask is out and it has not answered, or is asked
    /// `again`. What it answered before is kept until the new answer lands.
    pub(crate) fn ask(&self, account: &str, again: bool) -> Option<u64> {
        let mut answers = self.answers();
        let answer = answers.entry(account.to_owned()).or_default();
        if answer.asking.is_some() || (answer.value.is_some() && !again) {
            return None;
        }
        let ticket = self.tickets.fetch_add(1, Ordering::Relaxed) + 1;
        answer.asking = Some(ticket);
        Some(ticket)
    }

    /// The server answered the ask `ticket`; `false` when what is kept is what it answered before,
    /// so the page has nothing new to draw. An ask the account changed under is dropped: it was
    /// about something the account no longer has.
    pub(crate) fn answered(&self, account: &str, ticket: u64, value: T) -> bool {
        let mut answers = self.answers();
        let Some(answer) = answers.get_mut(account) else {
            return true;
        };
        if answer.asking != Some(ticket) {
            return true;
        }
        answer.asking = None;
        answer.value.replace(value.clone()) != Some(value)
    }

    /// What `account`'s server last answered.
    pub(crate) fn get(&self, account: &str) -> Option<T> {
        self.answers()
            .get(account)
            .and_then(|answer| answer.value.clone())
    }

    /// Drops what is kept for `account`, which changed or is gone, and any ask out.
    pub(crate) fn forget(&self, account: &str) {
        self.answers().remove(account);
    }
}
