// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! The entitlement this device last read, kept beside the store (`entitlement.md`).
//!
//! Read in the background and never on a path anyone waits for: at launch, after a sign-in, and
//! on a refresh, each only once the service's own interval has passed. Stored in the app data
//! directory rather than the keystore, because it is derived rather than secret and a keystore
//! read can cost a prompt. Two things are drawn from it: the Allodia notice's rise to Blocked in
//! the last week of a paid answer's grace, and whether the writing assistant is offered
//! (`allodia_ai.rs`).

use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use allodia_license::{
    AccountService, Answer, Cache, Capability, Error, Feature, Outcome, Stored,
};

use crate::{AllodiaGrantHealth, MailcalApp, allodia_transport::HttpsTransport};

/// The stored answer's file name, in the app data directory.
const FILE_NAME: &str = "allodia-entitlement.json";

/// The last answer, where it is kept, and whether a read is out.
#[derive(Debug, Default)]
pub(crate) struct EntitlementState {
    /// Where the answer is kept, or `None` for a core with nothing to keep (the showcase).
    path: Option<PathBuf>,
    cache: Mutex<Cache>,
    /// A read is out, so a second trigger does not start another.
    asking: AtomicBool,
}

impl EntitlementState {
    /// What the app data directory `dir` holds. A file that will not parse is nothing stored,
    /// which is the free app, never an error.
    pub(crate) fn load(dir: &std::path::Path) -> Self {
        let path = dir.join(FILE_NAME);
        let stored = std::fs::read_to_string(&path)
            .ok()
            .and_then(|body| serde_json::from_str::<Stored>(&body).ok());
        Self {
            path: Some(path),
            cache: Mutex::new(Cache::restore(stored)),
            ..Self::default()
        }
    }

    /// Whether the stored answer is in its last week of grace while granting a paid capability.
    pub(crate) fn lapses_soon(&self, now: i64) -> bool {
        self.cache
            .lock()
            .expect("entitlement lock")
            .lapses_soon(now)
    }

    /// Whether the answer in force at `now` grants `capability`: the stored one inside its grace,
    /// otherwise none.
    pub(crate) fn grants(&self, capability: &Capability, now: i64) -> bool {
        self.cache
            .lock()
            .expect("entitlement lock")
            .effective(now)
            .grants(capability)
    }

    fn should_refresh(&self, now: i64) -> bool {
        self.cache
            .lock()
            .expect("entitlement lock")
            .should_refresh(now)
    }

    /// Folds in what the last attempt came to: keeps a new answer, and erases the stored one when
    /// the service refused to give one.
    pub(crate) fn record(&self, outcome: Outcome, now: i64) {
        let mut cache = self.cache.lock().expect("entitlement lock");
        cache.apply(outcome, now);
        let Some(path) = &self.path else {
            return;
        };
        let Some(stored) = cache.stored() else {
            erase(path);
            return;
        };
        match serde_json::to_string(stored) {
            Ok(body) => {
                if let Err(err) = std::fs::write(path, body) {
                    log::warn!("allodia: the entitlement could not be stored; {err}");
                }
            }
            Err(err) => log::warn!("allodia: the entitlement could not be written out; {err}"),
        }
    }

    /// Forgets the answer, for an account that signed out: it described an account that is gone.
    pub(crate) fn forget(&self) {
        *self.cache.lock().expect("entitlement lock") = Cache::default();
        if let Some(path) = &self.path {
            erase(path);
        }
    }
}

/// Takes the stored answer off the disk, if there is one.
fn erase(path: &std::path::Path) {
    if path.exists()
        && let Err(err) = std::fs::remove_file(path)
    {
        log::warn!("allodia: the stored entitlement could not be erased; {err}");
    }
}

/// Asks for the entitlement with `token`, and once more with the token `fresh` mints when the
/// service refuses it, since the token held for the process may be one it no longer takes. A
/// refusal of the fresh token is the service's answer; a fresh token that cannot be had, like an
/// outage, learns nothing.
fn ask_twice(
    token: String,
    ask: impl Fn(&str) -> Result<Answer, Error>,
    fresh: impl FnOnce() -> Option<String>,
) -> Outcome {
    match ask(&token) {
        Err(Error::Unauthorized) => match fresh() {
            Some(token) => match ask(&token) {
                Err(Error::Unauthorized) => {
                    log::warn!("allodia: the account service refused to read the plan");
                    Outcome::Refused
                }
                read => outcome_of(read),
            },
            None => Outcome::Unreachable,
        },
        read => outcome_of(read),
    }
}

fn outcome_of(read: Result<Answer, Error>) -> Outcome {
    match read {
        Ok(answer) => {
            log::info!(
                "allodia: the plan was read; asking again in {} hour(s)",
                answer.refresh_after_seconds / 3600
            );
            Outcome::Answered(answer)
        }
        Err(err) => {
            log::info!("allodia: the plan could not be read; keeping what is stored ({err})");
            Outcome::Unreachable
        }
    }
}

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

impl MailcalApp {
    /// Reads the entitlement in the background when an Allodia account is signed in and the
    /// service's interval has passed. Returns at once. First tells the writing assistant's offer
    /// what the stored answer grants now, so a grace that has run out takes it away.
    pub(crate) fn refresh_allodia_entitlement(&self) {
        self.publish_ai_entitlement();
        let signed_in = self.allodia.lock().expect("allodia account lock").is_some();
        if signed_in && self.allodia_entitlement.should_refresh(now()) {
            self.read_allodia_entitlement_in_background();
        }
    }

    /// Reads the entitlement in the background now, whatever the interval says, unless a read is
    /// already out. Returns at once.
    pub(crate) fn read_allodia_entitlement_in_background(&self) {
        let state = &self.allodia_entitlement;
        // The app, for a read that outlives the call that started it.
        let Some(app) = self.rediscovery.app() else {
            return;
        };
        if state.asking.swap(true, Ordering::AcqRel) {
            return;
        }
        std::thread::spawn(move || {
            app.read_allodia_entitlement();
            app.allodia_entitlement
                .asking
                .store(false, Ordering::Release);
        });
    }

    /// One read, blocking. A failure to reach the service changes nothing, and neither does a
    /// grant the service refused at refresh, which is recorded as the sign-in it is. A read the
    /// service refuses with a fresh token grants nothing from then on.
    fn read_allodia_entitlement(&self) {
        let Ok(token) = self.allodia_access_token("the plan") else {
            return;
        };
        if !self.allodia_grant_permits(Feature::Entitlement) {
            self.note_allodia_health(AllodiaGrantHealth::NeedsReauth);
            return;
        }
        let Ok(transport) = HttpsTransport::new(self.runtime.handle().clone()) else {
            return;
        };
        let lapsed_before = self.allodia_entitlement.lapses_soon(now());
        let service = AccountService::new(allodia_license::host());
        let outcome = ask_twice(
            token,
            |token| service.entitlement(&transport, token),
            || {
                self.forget_allodia_access_token();
                self.allodia_access_token("the plan").ok()
            },
        );
        if outcome == Outcome::Refused {
            self.note_allodia_health(AllodiaGrantHealth::SignedOut);
        }
        self.allodia_entitlement.record(outcome, now());
        self.publish_ai_entitlement();
        if self.allodia_entitlement.lapses_soon(now()) != lapsed_before {
            self.app.signal_notices();
        }
    }
}

#[path = "allodia_ai.rs"]
mod ai;

#[cfg(test)]
#[path = "allodia_entitlement_tests.rs"]
mod tests;
