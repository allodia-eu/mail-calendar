//! Dialling again what a launch, or an earlier retry, could not reach.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use engine_api::AccountId;

use crate::{MailcalApp, account_registry, reconnect};

/// The ids being dialled again right now, each with whether a dial was asked for it meanwhile:
/// the dial under way was planned from what the account was before that ask.
pub(crate) type Redialling = Arc<Mutex<HashMap<String, bool>>>;

/// Whether a dial was asked for `id` while the one under way runs: that one was planned from
/// what the account was before, so it installs nothing and the claim queues the account again.
pub(crate) fn asked_again(redialling: &Redialling, id: &str) -> bool {
    lock(redialling).get(id).copied().unwrap_or(false)
}

/// Locks a set a [`Claim`] gives back to, poisoned or not: an id left claimed is never dialled
/// again, and one left out of `disconnected` is not either.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The ids a retry is dialling, released from `redialling` when the dial is over, however it
/// ends. An id a dial was asked for meanwhile goes back into `disconnected`, so it is dialled
/// again from what the account is now.
struct Claim {
    redialling: Redialling,
    disconnected: Arc<Mutex<HashSet<String>>>,
    ids: Vec<String>,
}

impl Claim {
    /// Releases the ids and answers whether any was asked for again meanwhile.
    fn release(&mut self) -> bool {
        let asked_again: Vec<String> = {
            let mut redialling = lock(&self.redialling);
            self.ids
                .drain(..)
                .filter(|id| redialling.remove(id).unwrap_or(false))
                .collect()
        };
        let any = !asked_again.is_empty();
        lock(&self.disconnected).extend(asked_again);
        any
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        self.release();
    }
}

impl MailcalApp {
    /// Re-runs the full connect for every account sitting **disconnected** (a boot outage kept
    /// as a placeholder, a prior retry that failed, or a use just switched), and for every
    /// account whose calendar or address book could not be reached beside it, and joins each
    /// that succeeds back into the app with live providers: so a recovered provider heals
    /// without an app restart, regaining every folder, its capabilities, and its calendar. A
    /// still-unreachable account is re-queued for the next attempt, and one whose sign-in was
    /// refused waits for a replaced credential. An account already being dialled is not dialled
    /// twice: one drained from `disconnected` is marked instead, and dialled again from what the
    /// account is now as soon as the dial under way ends. Fire-and-forget on the runtime, so a
    /// slow re-dial never blocks the host's `dispatch`. A no-op when there is nothing to dial.
    pub(crate) fn retry_connections(&self) {
        let mut ids: Vec<String> = lock(&self.disconnected).drain().collect();
        let queued: HashSet<String> = ids.iter().cloned().collect();
        // An account whose mail is up but whose calendar or address book could not be reached is
        // dialled again too: nothing else would, until the next launch.
        for id in self
            .app
            .accounts_with_use_problem(mailcal_app::UseProblem::Unreachable)
            .into_keys()
        {
            if !queued.contains(&id) {
                ids.push(id);
            }
        }
        // An account already being dialled is left to that dial. One that was queued is marked
        // as asked for again, since the dial under way was planned before it was.
        {
            let mut redialling = lock(&self.redialling);
            ids.retain(|id| match redialling.get_mut(id) {
                Some(asked_again) => {
                    *asked_again |= queued.contains(id);
                    false
                }
                None => {
                    redialling.insert(id.clone(), false);
                    true
                }
            });
        }
        if ids.is_empty() {
            return;
        }
        let mut claim = Claim {
            redialling: Arc::clone(&self.redialling),
            disconnected: Arc::clone(&self.disconnected),
            ids: ids.clone(),
        };
        // Snapshot each account's reconnection plan under the registry lock (so no lock is held
        // across the re-dial); an id whose registry entry is gone (removed meanwhile) drops out.
        // The catch-up sync after a successful reconnect resolves the account's own depth.
        let plans: Vec<(AccountId, account_registry::AccountDial)> = ids
            .into_iter()
            .filter_map(|id| {
                let account_id = AccountId::try_from(id.as_str()).ok()?;
                Some((account_id, self.registry.dial(&id)?))
            })
            .collect();
        if plans.is_empty() {
            return;
        }
        log::info!(
            "retry_connections: re-dialling {} account(s) with something unreachable",
            plans.len(),
        );
        let reconnect = reconnect::reconnect_all(
            Arc::clone(&self.app),
            Arc::clone(&self.background),
            Arc::clone(&self.registry),
            Arc::clone(&self.disconnected),
            Arc::clone(&self.redialling),
            plans,
            self.device_zone.clone(),
        );
        let app = self.rediscovery.app();
        self.runtime.spawn(async move {
            let unanswered = reconnect.await;
            let asked_again = claim.release();
            // Off the runtime: looking again blocks on discovery and on the move's connect, and
            // the last handle on the app must not be dropped on a thread of its own runtime.
            if let Some(app) = app {
                std::thread::spawn(move || {
                    if asked_again {
                        app.retry_connections();
                    }
                    for id in unanswered {
                        app.rediscover_servers(&id);
                    }
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        sync::{Arc, Mutex},
    };

    use super::Claim;

    fn claim_of(
        redialling: &super::Redialling,
        ids: &[&str],
    ) -> (Claim, Arc<Mutex<HashSet<String>>>) {
        let disconnected = Arc::new(Mutex::new(HashSet::new()));
        let claim = Claim {
            redialling: Arc::clone(redialling),
            disconnected: Arc::clone(&disconnected),
            ids: ids.iter().map(|&id| id.to_owned()).collect(),
        };
        (claim, disconnected)
    }

    #[test]
    fn a_claim_gives_its_ids_back_even_when_its_dial_panics() {
        let redialling = Arc::new(Mutex::new(HashMap::from([
            ("a".to_owned(), false),
            ("b".to_owned(), false),
        ])));
        let (claim, _) = claim_of(&redialling, &["a"]);
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _held = claim;
            panic!("the dial failed");
        }));
        assert!(unwound.is_err());
        assert_eq!(
            *redialling.lock().unwrap(),
            HashMap::from([("b".to_owned(), false)]),
            "only the claimed id is released"
        );
    }

    #[test]
    fn a_dial_asked_for_meanwhile_is_queued_again_when_the_claim_ends() {
        let redialling = Arc::new(Mutex::new(HashMap::from([
            ("asked".to_owned(), true),
            ("quiet".to_owned(), false),
        ])));
        let (mut claim, disconnected) = claim_of(&redialling, &["asked", "quiet"]);
        assert!(claim.release(), "one id was asked for again");
        assert!(redialling.lock().unwrap().is_empty());
        assert_eq!(
            *disconnected.lock().unwrap(),
            HashSet::from(["asked".to_owned()]),
            "only the id asked for again is queued, so a working account is not dialled twice"
        );
        assert!(
            !claim.release(),
            "a released claim gives nothing back twice"
        );
    }

    #[test]
    fn a_dial_asked_for_again_meanwhile_installs_nothing() {
        let redialling = Arc::new(Mutex::new(HashMap::from([
            ("asked".to_owned(), true),
            ("quiet".to_owned(), false),
        ])));
        assert!(super::asked_again(&redialling, "asked"));
        assert!(!super::asked_again(&redialling, "quiet"));
        assert!(!super::asked_again(&redialling, "unclaimed"));
    }
}
