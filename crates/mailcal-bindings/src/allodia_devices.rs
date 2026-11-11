//! What the person's other devices have to say about this device's accounts, kept by the core
//! (`docs/waiting.md`, `docs/settings.md` category 10).
//!
//! The core runs the pass itself, whenever what it compares may have moved: once somewhere to keep
//! the bookkeeping is installed, after an Allodia sign-in, after an account is added and after a
//! sharing change. A pass asked for while one runs runs once more after it, so nothing asked for
//! is lost; a pass that ends after the sign-in it ran for has gone says nothing, so a signed-out
//! device is never shown the last account's questions. What a client draws is
//! [`AllodiaDevices`], read again on each `Surface::Settings` signal.
#![cfg_attr(
    not(any(feature = "allodia-license", test)),
    expect(dead_code, reason = "only a build that can sign in runs a pass")
)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    AllodiaGrantHealth, ControlState,
    allodia_sync::{AllodiaAccountOffer, AllodiaSyncReport},
    sync_state::SyncBookkeeping,
};

/// Why the person's other devices could not be asked, or a sharing change did not take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaSyncFailure {
    /// The sign-in predates the permission the account list needs: signing in again grants it.
    NeedsReauth,
    /// The service refused the sign-in: the person is signed out.
    SignedOut,
    /// Anything else: the service could not be reached, or refused the change.
    Unavailable,
}

impl From<AllodiaGrantHealth> for AllodiaSyncFailure {
    /// What a failure is drawn as: the sign-in's health, never the failure's text.
    fn from(health: AllodiaGrantHealth) -> Self {
        match health {
            AllodiaGrantHealth::NeedsReauth => Self::NeedsReauth,
            AllodiaGrantHealth::SignedOut => Self::SignedOut,
            AllodiaGrantHealth::Ok => Self::Unavailable,
        }
    }
}

/// What happened elsewhere to one of this device's accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaElsewhere {
    /// Its settings were changed on another device.
    Changed,
    /// It was removed on another device.
    Removed,
}

/// One of this device's accounts that another device changed or removed, and the answer this
/// device can give: keep what it has, which pauses the account's sharing.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaDevicesQuestion {
    pub account_id: String,
    /// The address, which is what a person recognises.
    pub email: String,
    pub elsewhere: AllodiaElsewhere,
    /// Keep what this device has.
    pub keep: ControlState,
    /// Why the last answer did not take.
    pub failed: Option<AllodiaSyncFailure>,
}

/// What the person's other devices have to say: drawn above the accounts in Settings → Accounts,
/// and on the first-run card. Accounts changed elsewhere come before those removed elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaDevices {
    /// A pass is running.
    pub checking: bool,
    /// The last pass answered, and the other devices hold nothing this one lacks and ask nothing
    /// about it: Settings draws no card, and the first-run card may say there are no accounts.
    pub nothing_found: bool,
    /// Accounts the other devices hold that this one has not got.
    pub offers: Vec<AllodiaAccountOffer>,
    pub questions: Vec<AllodiaDevicesQuestion>,
    /// Why the last pass did not work.
    pub failure: Option<AllodiaSyncFailure>,
}

/// The bookkeeping, and what the passes have found.
#[derive(Debug, Default)]
pub(crate) struct Devices {
    bookkeeping: Mutex<Option<Arc<SyncBookkeeping>>>,
    state: Mutex<State>,
    /// Held across a pass and a sharing change: both write the bookkeeping and the service, and
    /// two at once would each act on what the other is changing.
    #[cfg_attr(
        not(feature = "allodia-license"),
        expect(dead_code, reason = "only a build that can sign in passes or changes sharing")
    )]
    pub(crate) writing: Mutex<()>,
    /// What a pass answers in a test, which never asks the service.
    #[cfg(all(test, feature = "allodia-license"))]
    pub(crate) canned: Mutex<Option<Result<AllodiaSyncReport, AllodiaSyncFailure>>>,
}

#[derive(Debug, Default)]
struct State {
    /// Grows with each sign-in and sign-out, so a pass that ends after either says nothing.
    session: u64,
    running: bool,
    /// A pass was asked for while one ran.
    again: bool,
    /// `None` until a pass has answered, which is not the same as one that found nothing.
    report: Option<AllodiaSyncReport>,
    failure: Option<AllodiaSyncFailure>,
    /// Each account whose last sharing change did not take.
    sharing_failed: HashMap<String, AllodiaSyncFailure>,
}

impl Devices {
    pub(crate) fn install(&self, bookkeeping: Arc<SyncBookkeeping>) {
        *self.bookkeeping.lock().expect("allodia devices lock") = Some(bookkeeping);
    }

    pub(crate) fn bookkeeping(&self) -> Option<Arc<SyncBookkeeping>> {
        self.bookkeeping
            .lock()
            .expect("allodia devices lock")
            .clone()
    }

    /// Asks for a pass: the session to run it for when the caller should start one now, `None`
    /// when one is already running, which then runs once more.
    pub(crate) fn want_pass(&self) -> Option<u64> {
        let mut state = self.state.lock().expect("allodia devices lock");
        if state.running {
            state.again = true;
            return None;
        }
        state.running = true;
        state.failure = None;
        Some(state.session)
    }

    /// The pass for `session` ended with `outcome`: kept while that sign-in still stands. Answers
    /// the session to run the next pass for, when one was asked for meanwhile.
    pub(crate) fn pass_ended(
        &self,
        session: u64,
        outcome: Result<AllodiaSyncReport, AllodiaSyncFailure>,
    ) -> Option<u64> {
        let mut state = self.state.lock().expect("allodia devices lock");
        if session == state.session {
            match outcome {
                Ok(report) => {
                    state.report = Some(report);
                    state.failure = None;
                }
                Err(failure) => state.failure = Some(failure),
            }
        }
        if std::mem::take(&mut state.again) {
            state.failure = None;
            return Some(state.session);
        }
        state.running = false;
        None
    }

    /// A sign-in or a sign-out: nothing found for the account that was signed in describes the
    /// next one. A pass still running finishes, and says nothing.
    pub(crate) fn new_session(&self) {
        let mut state = self.state.lock().expect("allodia devices lock");
        state.session += 1;
        state.report = None;
        state.failure = None;
        state.sharing_failed.clear();
    }

    /// A sharing change to `account` ended. One that took answers the questions about it.
    pub(crate) fn sharing_changed(&self, account: &str, outcome: Result<(), AllodiaSyncFailure>) {
        let mut state = self.state.lock().expect("allodia devices lock");
        match outcome {
            Ok(()) => {
                state.sharing_failed.remove(account);
                if let Some(report) = &mut state.report {
                    forget_questions(report, account);
                }
            }
            Err(failure) => {
                state.sharing_failed.insert(account.to_owned(), failure);
            }
        }
    }

    /// `account` was removed: nothing is left to ask about it.
    pub(crate) fn forget_account(&self, account: &str) {
        let mut state = self.state.lock().expect("allodia devices lock");
        state.sharing_failed.remove(account);
        if let Some(report) = &mut state.report {
            forget_questions(report, account);
        }
    }

    pub(crate) fn sharing_failed(&self, account: &str) -> Option<AllodiaSyncFailure> {
        self.state
            .lock()
            .expect("allodia devices lock")
            .sharing_failed
            .get(account)
            .copied()
    }

    /// What the card draws, each Keep drawn by `keep`; `None` while nothing is known, before the
    /// first pass has answered: a heading over an empty list would claim the other devices hold
    /// no accounts, which this device does not yet know.
    pub(crate) fn view(&self, keep: impl Fn(&str) -> ControlState) -> Option<AllodiaDevices> {
        let state = self.state.lock().expect("allodia devices lock");
        let mut offers = Vec::new();
        let mut questions = Vec::new();
        if let Some(report) = &state.report {
            offers.clone_from(&report.offers);
            let asked = [
                (&report.changed_elsewhere, AllodiaElsewhere::Changed),
                (&report.removed_elsewhere, AllodiaElsewhere::Removed),
            ];
            for (changes, elsewhere) in asked {
                questions.extend(changes.iter().map(|change| AllodiaDevicesQuestion {
                    account_id: change.account_id.clone(),
                    email: change.email.clone(),
                    elsewhere,
                    keep: keep(&change.account_id),
                    failed: state.sharing_failed.get(&change.account_id).copied(),
                }));
            }
        }
        let known = state.running || state.failure.is_some() || state.report.is_some();
        let nothing_found = !state.running
            && state.failure.is_none()
            && offers.is_empty()
            && questions.is_empty();
        known.then(|| AllodiaDevices {
            checking: state.running,
            nothing_found,
            offers,
            questions,
            failure: state.failure,
        })
    }
}

fn forget_questions(report: &mut AllodiaSyncReport, account: &str) {
    report
        .changed_elsewhere
        .retain(|change| change.account_id != account);
    report
        .removed_elsewhere
        .retain(|change| change.account_id != account);
}

#[cfg(test)]
#[path = "allodia_devices_tests.rs"]
mod tests;
