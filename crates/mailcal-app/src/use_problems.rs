//! What stopped an account's calendar or contacts from connecting while the account itself did.
//!
//! An account with mail is reachable or not by its mail alone (`docs/accounts.md` rule 7), so a
//! calendar that fails beside a working mailbox badges nothing. It still has to be said: the
//! account's page in Settings shows the use with what went wrong and the action that fixes it,
//! read from here, and an unreachable use is dialled again with the next retry.

use std::collections::BTreeMap;

use engine_api::AccountId;

use crate::{App, protocol::Surface};

/// Why a use the account is used for did not connect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseProblem {
    /// Its server does not take the account's sign-in, and needs one of its own: an OAuth grant
    /// the profile does not let reach it (`docs/mail-oauth.md` rule 10). Nothing was sent.
    NeedsOwnSignIn,
    /// Its server answered and refused the account's sign-in.
    SignInRefused,
    /// Its server could not be reached, or did not answer in time.
    Unreachable,
}

/// An account's calendar and contacts problems; both `None` when each connected or was not tried.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UseProblems {
    /// Why the calendar did not connect.
    pub calendar: Option<UseProblem>,
    /// Why the address book did not connect.
    pub contacts: Option<UseProblem>,
}

impl UseProblems {
    /// Whether either use failed with `problem`.
    #[must_use]
    pub fn any(&self, problem: UseProblem) -> bool {
        self.calendar == Some(problem) || self.contacts == Some(problem)
    }
}

impl<P> App<P> {
    /// Records what `account`'s latest dial made of its calendar and contacts, replacing what the
    /// one before it said. Signals [`Surface::Connectivity`] only when that changes.
    pub fn note_use_problems(&self, account: &AccountId, problems: UseProblems) {
        let changed = {
            let mut map = self
                .use_problems
                .lock()
                .expect("use-problems mutex poisoned");
            let before = map.get(account.as_str()).copied().unwrap_or_default();
            if problems == UseProblems::default() {
                map.remove(account.as_str());
            } else {
                map.insert(account.as_str().to_owned(), problems);
            }
            before != problems
        };
        if changed {
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// Forgets what `account`'s calendar, or its contacts, last failed with, once it is no longer
    /// used for it: a use switched off has nothing left to fix, and the next dial would say so
    /// only after it has run.
    pub fn forget_use_problem(&self, account: &AccountId, calendar: bool) {
        let mut problems = self.use_problems(account);
        if calendar {
            problems.calendar = None;
        } else {
            problems.contacts = None;
        }
        self.note_use_problems(account, problems);
    }

    /// What `account`'s calendar and contacts last failed with.
    #[must_use]
    pub fn use_problems(&self, account: &AccountId) -> UseProblems {
        self.use_problems
            .lock()
            .expect("use-problems mutex poisoned")
            .get(account.as_str())
            .copied()
            .unwrap_or_default()
    }

    /// Every account with a use that failed, keyed by id.
    #[must_use]
    pub fn every_use_problem(&self) -> BTreeMap<String, UseProblems> {
        self.use_problems
            .lock()
            .expect("use-problems mutex poisoned")
            .clone()
    }

    /// Every account with a use that failed with `problem`, keyed by id.
    #[must_use]
    pub fn accounts_with_use_problem(&self, problem: UseProblem) -> BTreeMap<String, UseProblems> {
        self.use_problems
            .lock()
            .expect("use-problems mutex poisoned")
            .iter()
            .filter(|(_, problems)| problems.any(problem))
            .map(|(id, problems)| (id.clone(), *problems))
            .collect()
    }
}

#[cfg(test)]
#[path = "use_problems_tests.rs"]
mod tests;
