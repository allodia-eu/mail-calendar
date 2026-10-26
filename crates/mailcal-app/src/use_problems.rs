//! What stopped an account's calendar or contacts from connecting while the account itself did.
//!
//! An account with mail is reachable or not by its mail alone (`docs/accounts.md` rule 7), so a
//! calendar that fails beside a working mailbox badges nothing. It still has to be said: the
//! account's page in Settings shows the use with what went wrong and the action that fixes it,
//! read from here, and an unreachable use is dialled again with the next retry.
//!
//! A use whose servers were looked for again and found on another site is paused until the
//! person allows the new one (`docs/account-autodetect.md` rule 14): the change waits here,
//! beside the problem it explains, so a dial that fails again keeps the use paused and one that
//! connects after all lifts the pause.

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
    /// Its server is gone, and looking again found it on another site, which the person has not
    /// yet allowed ([`ServerChange`]). Nothing is sent to the new server until they do.
    ServerChanged,
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

    /// Only the uses paused for a server change: what is left when the whole account failed.
    /// The account's own prompt replaces every other problem, but a pause waits on the person,
    /// and a failed dial is no answer to it.
    #[must_use]
    pub fn paused_only(&self) -> Self {
        let paused = |problem| {
            (problem == Some(UseProblem::ServerChanged))
                .then_some(problem)
                .flatten()
        };
        Self {
            calendar: paused(self.calendar),
            contacts: paused(self.contacts),
        }
    }
}

/// Servers found on another site for an account's calendar or contacts, waiting for the person
/// to allow them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerChange {
    /// The site (registrable domain) the use connected to before.
    pub previous_site: String,
    /// The site the new servers are on.
    pub proposed_site: String,
    /// The calendar server found, when the calendar moved.
    pub calendar_url: Option<String>,
    /// The address-book server found, when the contacts moved.
    pub contacts_url: Option<String>,
}

/// Every account's use problems, and the server changes some of them wait on.
#[derive(Debug, Default)]
pub(crate) struct UseState {
    problems: BTreeMap<String, UseProblems>,
    changes: BTreeMap<String, ServerChange>,
}

/// What a use's problem becomes when a dial reports `now` and the use was `before`: a paused use
/// stays paused while it still does not connect, and one that connects is no longer paused.
fn held(before: Option<UseProblem>, now: Option<UseProblem>) -> Option<UseProblem> {
    match (before, now) {
        (
            Some(UseProblem::ServerChanged),
            Some(UseProblem::Unreachable | UseProblem::ServerChanged),
        ) => Some(UseProblem::ServerChanged),
        (_, now) => now,
    }
}

impl UseState {
    /// Stores `problems` for `account`, keeping a pause that still applies; reports whether
    /// anything changed.
    fn note(&mut self, account: &str, problems: UseProblems) -> bool {
        let before = self.problems.get(account).copied().unwrap_or_default();
        let after = UseProblems {
            calendar: held(before.calendar, problems.calendar),
            contacts: held(before.contacts, problems.contacts),
        };
        if !after.any(UseProblem::ServerChanged) {
            self.changes.remove(account);
        }
        if after == UseProblems::default() {
            self.problems.remove(account);
        } else {
            self.problems.insert(account.to_owned(), after);
        }
        before != after
    }
}

impl<P> App<P> {
    /// Records what `account`'s latest dial made of its calendar and contacts, replacing what the
    /// one before it said, except that a use paused for a server change stays paused while it
    /// still does not connect. Signals [`Surface::Connectivity`] only when that changes.
    pub fn note_use_problems(&self, account: &AccountId, problems: UseProblems) {
        let changed = self
            .use_problems
            .lock()
            .expect("use-problems mutex poisoned")
            .note(account.as_str(), problems);
        if changed {
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// Pauses `account`'s calendar or contacts, whichever `change` moves, until the person allows
    /// the servers found on another site, or the use connects again by itself. Signals
    /// [`Surface::Connectivity`] only when that changes.
    pub fn note_server_change(&self, account: &AccountId, change: ServerChange) {
        let mut state = self
            .use_problems
            .lock()
            .expect("use-problems mutex poisoned");
        let mut problems = state
            .problems
            .get(account.as_str())
            .copied()
            .unwrap_or_default();
        if change.calendar_url.is_some() {
            problems.calendar = Some(UseProblem::ServerChanged);
        }
        if change.contacts_url.is_some() {
            problems.contacts = Some(UseProblem::ServerChanged);
        }
        if !problems.any(UseProblem::ServerChanged) {
            return;
        }
        let unchanged = state.problems.get(account.as_str()) == Some(&problems)
            && state.changes.get(account.as_str()) == Some(&change);
        if unchanged {
            return;
        }
        state.problems.insert(account.as_str().to_owned(), problems);
        state.changes.insert(account.as_str().to_owned(), change);
        drop(state);
        self.observer.surface_changed(Surface::Connectivity);
    }

    /// The server change `account` waits on, if it waits on one.
    #[must_use]
    pub fn server_change(&self, account: &AccountId) -> Option<ServerChange> {
        self.use_problems
            .lock()
            .expect("use-problems mutex poisoned")
            .changes
            .get(account.as_str())
            .cloned()
    }

    /// Forgets the change `account` waited on, once the person allowed it: what its uses report
    /// next comes from the dial of the new servers. Only `allowed` is forgotten: a lookup may have
    /// found another change while the allowed one was connecting, and nobody has allowed that.
    pub fn forget_server_change(&self, account: &AccountId, allowed: &ServerChange) {
        let changed = {
            let mut state = self
                .use_problems
                .lock()
                .expect("use-problems mutex poisoned");
            if state.changes.get(account.as_str()) != Some(allowed) {
                return;
            }
            state.changes.remove(account.as_str());
            let before = state
                .problems
                .get(account.as_str())
                .copied()
                .unwrap_or_default();
            let unpause = |problem| {
                (problem != Some(UseProblem::ServerChanged))
                    .then_some(problem)
                    .flatten()
            };
            let after = UseProblems {
                calendar: unpause(before.calendar),
                contacts: unpause(before.contacts),
            };
            state.note(account.as_str(), after)
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
            .problems
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
            .problems
            .clone()
    }

    /// Every account with a use that failed with `problem`, keyed by id.
    #[must_use]
    pub fn accounts_with_use_problem(&self, problem: UseProblem) -> BTreeMap<String, UseProblems> {
        self.use_problems
            .lock()
            .expect("use-problems mutex poisoned")
            .problems
            .iter()
            .filter(|(_, problems)| problems.any(problem))
            .map(|(id, problems)| (id.clone(), *problems))
            .collect()
    }
}

#[cfg(test)]
#[path = "use_problems_tests.rs"]
mod tests;
