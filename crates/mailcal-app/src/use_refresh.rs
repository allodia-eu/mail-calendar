//! What a refresh says about a calendar or address book that connected beside a working account.
//!
//! A dial records why a use did not connect. A use that connected and then stops answering is
//! dialled by nobody, so its refreshes say it instead (`docs/accounts.md` rule 16): two refreshes
//! in a row that reached nothing record it [`UseProblem::Unreachable`], as a dial that could not
//! reach it does, so one lost request is not an outage; a refresh that reaches the server takes
//! it away. Unreachable is the only problem a refresh raises or clears, since every other one is
//! an answer only a dial gets. A refresh while the device is offline says nothing about the
//! server, and counts for nothing.

use engine_api::{AccountId, Provider};

use super::{UseProblem, UseProblems, UseState};
use crate::{App, protocol::Surface};

/// How many refreshes in a row must reach nothing before the use is recorded unreachable.
pub(crate) const MISSES_BEFORE_UNREACHABLE: u8 = 2;

/// Which use a refresh synced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefreshedUse {
    /// The account's calendar.
    Calendar,
    /// The account's address books.
    Contacts,
}

/// Refreshes in a row that reached nothing, per use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Misses {
    calendar: u8,
    contacts: u8,
}

impl UseState {
    /// Folds one refresh of `account`'s `refreshed` use, which `reached` its server or did not,
    /// into what is recorded. Reports whether the problems changed.
    fn refreshed(&mut self, account: &str, refreshed: RefreshedUse, reached: bool) -> bool {
        let before = self.problems.get(account).copied().unwrap_or_default();
        let mut misses = self.misses.get(account).copied().unwrap_or_default();
        let (problem, count) = match refreshed {
            RefreshedUse::Calendar => (before.calendar, &mut misses.calendar),
            RefreshedUse::Contacts => (before.contacts, &mut misses.contacts),
        };
        let now = if reached {
            *count = 0;
            problem.filter(|problem| *problem != UseProblem::Unreachable)
        } else if problem.is_some() {
            *count = 0;
            problem
        } else {
            *count = count.saturating_add(1);
            (*count >= MISSES_BEFORE_UNREACHABLE).then(|| {
                *count = 0;
                UseProblem::Unreachable
            })
        };
        if misses == Misses::default() {
            self.misses.remove(account);
        } else {
            self.misses.insert(account.to_owned(), misses);
        }
        let after = match refreshed {
            RefreshedUse::Calendar => UseProblems {
                calendar: now,
                ..before
            },
            RefreshedUse::Contacts => UseProblems {
                contacts: now,
                ..before
            },
        };
        if after == before {
            return false;
        }
        if after == UseProblems::default() {
            self.problems.remove(account);
        } else {
            self.problems.insert(account.to_owned(), after);
        }
        true
    }
}

impl<P: Provider> App<P> {
    /// Records what a refresh of `account`'s `refreshed` use learned: `reached` is the pass's
    /// verdict (`sync_account::reachability`), `None` when it says nothing. Signals
    /// [`Surface::Connectivity`] and returns `true` only when the use's problem changed.
    pub(crate) fn note_use_refreshed(
        &self,
        account: &AccountId,
        refreshed: RefreshedUse,
        reached: Option<bool>,
    ) -> bool {
        let Some(reached) = reached else {
            return false;
        };
        if !reached && !self.is_online() {
            return false;
        }
        let changed = self
            .use_problems
            .lock()
            .expect("use-problems mutex poisoned")
            .refreshed(account.as_str(), refreshed, reached);
        if changed {
            self.observer.surface_changed(Surface::Connectivity);
        }
        changed
    }
}

#[cfg(test)]
#[path = "use_refresh_tests.rs"]
mod tests;
