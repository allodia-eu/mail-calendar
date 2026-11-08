// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! The entitlement this device last read, kept beside the store (`entitlement.md`).
//!
//! Read in the background and never on a path anyone waits for: at launch, after a sign-in, and
//! on a refresh, each only once the service's own interval has passed. Stored in the app data
//! directory rather than the keystore, because it is derived rather than secret and a keystore
//! read can cost a prompt. Today the only thing drawn from it is the Allodia notice's rise to
//! Blocked in the last week of a paid answer's grace.

use std::{
    path::PathBuf,
    sync::{
        Mutex, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

use allodia_license::{AccountService, Cache, Error, Feature, Outcome, Stored};

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
    /// The app, for a read that outlives the call that started it.
    app: OnceLock<Weak<MailcalApp>>,
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

    /// Hands the state the app it belongs to, once the app exists.
    pub(crate) fn adopt(&self, app: &std::sync::Arc<MailcalApp>) {
        let _ = self.app.set(std::sync::Arc::downgrade(app));
    }

    /// Whether the stored answer is in its last week of grace while granting a paid capability.
    pub(crate) fn lapses_soon(&self, now: i64) -> bool {
        self.cache
            .lock()
            .expect("entitlement lock")
            .lapses_soon(now)
    }

    fn should_refresh(&self, now: i64) -> bool {
        self.cache
            .lock()
            .expect("entitlement lock")
            .should_refresh(now)
    }

    /// Folds in what the last attempt came to, and keeps a new answer.
    pub(crate) fn record(&self, outcome: Outcome, now: i64) {
        let mut cache = self.cache.lock().expect("entitlement lock");
        cache.apply(outcome, now);
        let (Some(path), Some(stored)) = (&self.path, cache.stored()) else {
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
        if let Some(path) = &self.path
            && path.exists()
            && let Err(err) = std::fs::remove_file(path)
        {
            log::warn!("allodia: the stored entitlement could not be erased; {err}");
        }
    }
}

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

impl MailcalApp {
    /// Reads the entitlement in the background when an Allodia account is signed in and the
    /// service's interval has passed. Returns at once.
    pub(crate) fn refresh_allodia_entitlement(&self) {
        let state = &self.allodia_entitlement;
        let signed_in = self.allodia.lock().expect("allodia account lock").is_some();
        if !signed_in || !state.should_refresh(now()) {
            return;
        }
        let Some(app) = state.app.get().and_then(Weak::upgrade) else {
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

    /// One read, blocking. A failure to reach the service changes nothing; a refused grant is
    /// recorded as the sign-in it is, and changes nothing about the stored answer either.
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
        let outcome = match AccountService::new(allodia_license::host())
            .entitlement(&transport, &token)
        {
            Ok(answer) => {
                log::info!(
                    "allodia: the plan was read; asking again in {} hour(s)",
                    answer.refresh_after_seconds / 3600
                );
                Outcome::Answered(answer)
            }
            Err(Error::Unauthorized) => {
                self.note_allodia_health(AllodiaGrantHealth::SignedOut);
                Outcome::Unreachable
            }
            Err(err) => {
                log::info!("allodia: the plan could not be read; keeping what is stored ({err})");
                Outcome::Unreachable
            }
        };
        self.allodia_entitlement.record(outcome, now());
        if self.allodia_entitlement.lapses_soon(now()) != lapsed_before {
            self.app.signal_notices();
        }
    }
}

#[cfg(test)]
#[path = "allodia_entitlement_tests.rs"]
mod tests;
