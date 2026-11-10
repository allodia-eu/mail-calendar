//! The browser sign-in the core is running for an account (`docs/provider-oauth.md` rule 13):
//! one at a time, a fresh one superseding the one still out, every step of it here rather than
//! in a client. What it returns is never handed to a client: the handle that completes it and a
//! Nextcloud grant stay on this side.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::{AccountCapability, BrowserSignIn, UnreachedUses};

#[path = "account_page_browser_out.rs"]
mod out;
pub(crate) use out::Out;
use out::{record, working};

/// How long a sign-in waits on the browser before it is over: what a closed browser tab costs,
/// since closing one tells nobody. The question before the browser waits until it is answered,
/// since it is on screen, or a fresh press replaces it.
pub(crate) const WAIT_CAP: Duration = Duration::from_mins(5);

/// How long the control that started a sign-in stays running once the browser is sent out: a
/// browser takes a moment to come up, and once it has, a fresh press starts the sign-in again, as
/// after a tab closed by accident.
pub(crate) const OPENING: Duration = Duration::from_secs(2);

/// The handle a started sign-in is completed with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pending {
    /// A Microsoft or Google account's consent.
    Consent(String),
    /// A standards or JMAP account's sign-in at its provider.
    Account(String),
    /// A Nextcloud Login Flow, polled by the core.
    Nextcloud(String),
}

/// What a sign-in's server answered when it was started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Began {
    pub(crate) pending: Pending,
    pub(crate) url: String,
    /// Where the browser returns; `None` for a Nextcloud sign-in, which returns nothing.
    pub(crate) redirect_uri: Option<String>,
    /// The uses a switch to the provider would cost (`docs/accounts.md` rule 18).
    pub(crate) unreached: Vec<AccountCapability>,
}

/// What a completion is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Completion {
    Consent {
        pending: String,
        callback_url: String,
    },
    Account {
        pending: String,
        callback_url: String,
        unreached: Option<UnreachedUses>,
    },
    Nextcloud {
        grant: String,
    },
}

/// A completion taken to run, with what its outcome is said with.
#[derive(Debug)]
pub(crate) struct Run {
    pub(crate) account: String,
    pub(crate) switching: bool,
    pub(crate) completion: Completion,
}

#[derive(Debug)]
enum Stage {
    Starting,
    Asking {
        began: Began,
    },
    InBrowser {
        began: Began,
        unreached: Option<UnreachedUses>,
        /// When the browser was sent out.
        since: Instant,
    },
    /// The completion waits in the page queue until it is taken.
    Completing(Option<Completion>),
}

#[derive(Debug)]
struct Attempt {
    id: u64,
    account: String,
    adding: Vec<AccountCapability>,
    switching: bool,
    stage: Stage,
    /// Grows with each stage, so a wait that outlived its cap ends only the stage it timed.
    serial: u64,
}

#[derive(Debug, Default)]
struct Slot {
    current: Option<Attempt>,
    next: u64,
    serial: u64,
    /// A Nextcloud grant whose completion failed, kept so the next sign-in of that account
    /// completes it without a second trip to the browser (`docs/nextcloud-sign-in.md` rule 6).
    kept: Option<(String, String)>,
    /// A shorter cap than [`WAIT_CAP`], for a test.
    cap: Option<Duration>,
    /// Another time than [`OPENING`], for a test.
    opening: Option<Duration>,
}

/// What starting a sign-in came to.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Start {
    /// It is the sign-in out now: begin it.
    Begin {
        attempt: u64,
        /// Another account's kept grant, to withdraw: it will never become an account now.
        withdraw: Option<String>,
    },
    /// The account's kept grant completes it, with no browser: queue the completion.
    Complete { attempt: u64 },
    /// One is being completed, which nothing supersedes.
    Refused,
}

/// The browser sign-in, and the grant kept from the last one.
#[derive(Debug, Default)]
pub(crate) struct BrowserSignIns {
    slot: Mutex<Slot>,
}

impl BrowserSignIns {
    fn slot(&self) -> std::sync::MutexGuard<'_, Slot> {
        self.slot.lock().expect("browser sign-in mutex poisoned")
    }

    /// Starts a sign-in of `account`, asking for `adding` besides what it is used for, in place of
    /// the one out. `switching`: it moves the account from a password to its provider.
    pub(crate) fn start(
        &self,
        account: &str,
        adding: &[AccountCapability],
        switching: bool,
    ) -> Start {
        let mut slot = self.slot();
        if matches!(
            slot.current,
            Some(Attempt {
                stage: Stage::Completing(_),
                ..
            })
        ) {
            return Start::Refused;
        }
        slot.next += 1;
        slot.serial += 1;
        let (attempt, serial) = (slot.next, slot.serial);
        let kept = slot.kept.take();
        let (stage, withdraw) = match kept {
            Some((owner, grant)) if owner == account => (
                Stage::Completing(Some(Completion::Nextcloud { grant })),
                None,
            ),
            other => (Stage::Starting, other.map(|(_, grant)| grant)),
        };
        let completes = matches!(stage, Stage::Completing(_));
        slot.current = Some(Attempt {
            id: attempt,
            account: account.to_owned(),
            adding: adding.to_vec(),
            switching,
            stage,
            serial,
        });
        if completes {
            Start::Complete { attempt }
        } else {
            Start::Begin { attempt, withdraw }
        }
    }

    /// The server started `attempt`: it asks the question first when the switch would cost a
    /// use, and waits on the browser otherwise. Answers the serial of the wait it entered, or
    /// `None` when the attempt is no longer the one out.
    pub(crate) fn began(&self, attempt: u64, began: Began) -> Option<u64> {
        let mut slot = self.slot();
        slot.serial += 1;
        let serial = slot.serial;
        let current = slot
            .current
            .as_mut()
            .filter(|current| current.id == attempt)?;
        if !matches!(current.stage, Stage::Starting) {
            return None;
        }
        current.stage = if began.unreached.is_empty() {
            Stage::InBrowser {
                began,
                unreached: None,
                since: Instant::now(),
            }
        } else {
            Stage::Asking { began }
        };
        current.serial = serial;
        Some(serial)
    }

    /// The person answered `attempt`'s question: on to the browser with the answer, or, with
    /// none, the sign-in is over. Answers the serial of the browser's wait.
    pub(crate) fn answered(&self, attempt: u64, answer: Option<UnreachedUses>) -> Option<u64> {
        let mut slot = self.slot();
        slot.serial += 1;
        let serial = slot.serial;
        let current = slot.current.take_if(|current| {
            current.id == attempt && matches!(current.stage, Stage::Asking { .. })
        })?;
        let (Some(answer), Stage::Asking { began }) = (answer, current.stage) else {
            return None;
        };
        slot.current = Some(Attempt {
            stage: Stage::InBrowser {
                began,
                unreached: Some(answer),
                since: Instant::now(),
            },
            serial,
            ..current
        });
        Some(serial)
    }

    /// The browser returned to `attempt`'s redirect: it is completed next, as a change on the
    /// page. Answers whether to queue that completion.
    pub(crate) fn returned(&self, attempt: u64, callback_url: String) -> bool {
        let mut slot = self.slot();
        let Some(current) = slot
            .current
            .as_mut()
            .filter(|current| current.id == attempt)
        else {
            return false;
        };
        let completion = match &current.stage {
            Stage::InBrowser {
                began, unreached, ..
            } => match &began.pending {
                Pending::Consent(pending) => Completion::Consent {
                    pending: pending.clone(),
                    callback_url,
                },
                Pending::Account(pending) => Completion::Account {
                    pending: pending.clone(),
                    callback_url,
                    unreached: *unreached,
                },
                Pending::Nextcloud(_) => return false,
            },
            _ => return false,
        };
        current.stage = Stage::Completing(Some(completion));
        true
    }

    /// `attempt`'s Nextcloud server granted access: it is completed next. Answers `false` for a
    /// sign-in that is over, whose grant the caller withdraws.
    pub(crate) fn granted(&self, attempt: u64, grant: String) -> bool {
        let mut slot = self.slot();
        let Some(current) = slot.current.as_mut().filter(|current| {
            current.id == attempt
                && matches!(
                    current.stage,
                    Stage::InBrowser {
                        began: Began {
                            pending: Pending::Nextcloud(_),
                            ..
                        },
                        ..
                    }
                )
        }) else {
            return false;
        };
        current.stage = Stage::Completing(Some(Completion::Nextcloud { grant }));
        true
    }

    /// The handle to poll `attempt`'s Nextcloud server with, while it waits on the browser.
    pub(crate) fn polling(&self, attempt: u64) -> Option<String> {
        match &self.slot().current {
            Some(Attempt {
                id,
                stage:
                    Stage::InBrowser {
                        began:
                            Began {
                                pending: Pending::Nextcloud(pending),
                                ..
                            },
                        ..
                    },
                ..
            }) if *id == attempt => Some(pending.clone()),
            _ => None,
        }
    }

    /// Takes `attempt`'s completion to run it.
    pub(crate) fn take(&self, attempt: u64) -> Option<Run> {
        let mut slot = self.slot();
        let current = slot
            .current
            .as_mut()
            .filter(|current| current.id == attempt)?;
        let Stage::Completing(completion) = &mut current.stage else {
            return None;
        };
        Some(Run {
            account: current.account.clone(),
            switching: current.switching,
            completion: completion.take()?,
        })
    }

    /// `attempt`'s completion ended: the sign-in is over, keeping `grant` when a Nextcloud
    /// completion failed in a way another try may not.
    pub(crate) fn completed(&self, attempt: u64, grant: Option<String>) {
        let mut slot = self.slot();
        let Some(current) = slot.current.take_if(|current| current.id == attempt) else {
            return;
        };
        slot.kept = grant.map(|grant| (current.account, grant));
    }

    /// Ends `attempt` while it has not reached its completion: cancelled, timed out, refused by
    /// its server, or its browser could not open. Answers whether it was the sign-in out, and so
    /// whether its end needs saying.
    pub(crate) fn end(&self, attempt: u64) -> Option<Ended> {
        let mut slot = self.slot();
        let current = slot.current.take_if(|current| {
            current.id == attempt && !matches!(current.stage, Stage::Completing(_))
        })?;
        Some(Ended {
            account: current.account,
            switching: current.switching,
        })
    }

    /// Ends `attempt` if it is still at the wait `serial` timed.
    pub(crate) fn timed_out(&self, attempt: u64, serial: u64) -> Option<Ended> {
        if self.at(attempt, serial) {
            self.end(attempt)
        } else {
            None
        }
    }

    /// Whether `attempt` is the sign-in out, still at the stage `serial` names.
    pub(crate) fn at(&self, attempt: u64, serial: u64) -> bool {
        self.slot()
            .current
            .as_ref()
            .is_some_and(|current| current.id == attempt && current.serial == serial)
    }

    /// Whether `attempt` waits on its browser for a redirect, rather than for its server.
    pub(crate) fn redirects(&self, attempt: u64) -> bool {
        matches!(
            &self.slot().current,
            Some(Attempt {
                id,
                stage: Stage::InBrowser { began, .. },
                ..
            }) if *id == attempt && began.redirect_uri.is_some()
        )
    }

    /// Forgets `account`'s sign-in and its kept grant, the account being gone; answers the grant,
    /// to withdraw.
    pub(crate) fn forget(&self, account: &str) -> Option<String> {
        let mut slot = self.slot();
        if slot
            .current
            .as_ref()
            .is_some_and(|current| current.account == account)
        {
            slot.current = None;
        }
        slot.kept
            .take_if(|(owner, _)| owner == account)
            .map(|(_, grant)| grant)
    }

    /// The sign-in out, as a client draws and drives it.
    pub(crate) fn record(&self) -> Option<BrowserSignIn> {
        self.slot().current.as_ref().map(record)
    }

    /// The sign-in out for `account`.
    pub(crate) fn of(&self, account: &str) -> Option<Out> {
        let slot = self.slot();
        let opening = slot.opening.unwrap_or(OPENING);
        slot.current
            .as_ref()
            .filter(|current| current.account == account)
            .map(|current| Out {
                record: record(current),
                adding: current.adding.clone(),
                working: working(current, opening),
            })
    }

    /// The account whose sign-in is out while the control that started it is running.
    pub(crate) fn working_account(&self) -> Option<String> {
        let slot = self.slot();
        let opening = slot.opening.unwrap_or(OPENING);
        slot.current
            .as_ref()
            .filter(|current| working(current, opening))
            .map(|current| current.account.clone())
    }

    /// How long the control that started a sign-in stays running once its browser is sent out.
    pub(crate) fn opening(&self) -> Duration {
        self.slot().opening.unwrap_or(OPENING)
    }

    #[cfg(test)]
    pub(crate) fn set_opening(&self, opening: Duration) {
        self.slot().opening = Some(opening);
    }

    /// How long a sign-in waits on its browser.
    pub(crate) fn cap(&self) -> Duration {
        self.slot().cap.unwrap_or(WAIT_CAP)
    }

    #[cfg(test)]
    pub(crate) fn set_cap(&self, cap: Duration) {
        self.slot().cap = Some(cap);
    }

    /// The account `attempt` signs in, while it is the one out.
    pub(crate) fn account_of(&self, attempt: u64) -> Option<String> {
        self.slot()
            .current
            .as_ref()
            .filter(|current| current.id == attempt)
            .map(|current| current.account.clone())
    }
}

/// A sign-in that ended before its completion.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Ended {
    pub(crate) account: String,
    pub(crate) switching: bool,
}

#[cfg(test)]
#[path = "account_page_browser_tests.rs"]
mod tests;
