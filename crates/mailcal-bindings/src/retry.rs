//! Dialling again what a launch, or an earlier retry, could not reach.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use engine_api::AccountId;

use crate::{MailcalApp, account_registry, reconnect, reconnect::Reconnected};

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
/// again from what the account is now. It also holds the shared stores waiting for their
/// parent's dial, so that none taken from the queue is lost however the dial ends.
struct Claim {
    redialling: Redialling,
    disconnected: Arc<Mutex<HashSet<String>>>,
    ids: Vec<String>,
    after_parent: Vec<Waiting>,
}

/// A shared store waiting for its parent's dial.
struct Waiting {
    store: String,
    parent: String,
    /// Whether it was taken from the queue, rather than derived from its parent being dialled.
    from_queue: bool,
}

impl Claim {
    /// Releases the ids and settles the shared stores waiting for their parent, answering what
    /// is to be dialled at once, which it queues: the ids asked for again meanwhile, and the
    /// stores whose parent the dial connected.
    ///
    /// A store follows what `dialled`, the dial's outcome, says of its parent: dialled at once
    /// when it connected the parent; queued with a parent it queued again (one it could not reach,
    /// or reached only over plaintext); and otherwise dropped, as with a parent whose sign-in was
    /// refused, which waits for a new credential the store signs in with too (replacing it dials
    /// the parent's stores). With no outcome, the dial never ran: a store taken from the queue
    /// goes back, unless another pass is dialling its parent, and one derived from its parent is
    /// dropped. A parent's dial, or the dial it is asked for again after, derives the store again
    /// and settles it by that dial's outcome; put back, it would be dialled alone after a refusal.
    fn release(&mut self, dialled: Option<&Reconnected>) -> Vec<String> {
        let (mut again, being_dialled): (Vec<String>, HashSet<String>) = {
            let mut redialling = lock(&self.redialling);
            let again = self
                .ids
                .drain(..)
                .filter(|id| redialling.remove(id).unwrap_or(false))
                .collect();
            (again, redialling.keys().cloned().collect())
        };
        let mut queued = lock(&self.disconnected);
        queued.extend(again.iter().cloned());
        for waiting in self.after_parent.drain(..) {
            match dialled {
                Some(dialled) if dialled.connected.contains(&waiting.parent) => {
                    queued.insert(waiting.store.clone());
                    again.push(waiting.store);
                }
                Some(dialled) if dialled.requeued.contains(&waiting.parent) => {
                    queued.insert(waiting.store);
                }
                None if waiting.from_queue && !being_dialled.contains(&waiting.parent) => {
                    queued.insert(waiting.store);
                }
                Some(_) | None => {}
            }
        }
        again
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        self.release(None);
    }
}

impl MailcalApp {
    /// Re-runs the full connect for every account sitting **disconnected** (a boot outage kept
    /// as a placeholder, a prior retry that failed, or a use just switched), and joins each that
    /// succeeds back into the app with live providers: so a recovered provider heals without an
    /// app restart, regaining every folder, its capabilities, and its calendar. A
    /// still-unreachable account is re-queued for the next attempt, and one whose sign-in was
    /// refused waits for a replaced credential. An account whose calendar or address book could
    /// not be reached beside a working mailbox has those two dialled again and nothing else
    /// (`reconnect::reconnect_uses`). An account already being dialled is not dialled
    /// twice: one drained from `disconnected` is marked instead, and dialled again from what the
    /// account is now as soon as the dial under way ends.
    /// Fire-and-forget on the runtime, so a slow re-dial never blocks the host's `dispatch`. A
    /// no-op when there is nothing to dial.
    pub(crate) fn retry_connections(&self) {
        let ids: Vec<String> = lock(&self.disconnected).drain().collect();
        self.dial_from_queue(ids, true);
    }

    /// Dials those of `ids` still queued, and nothing else: no other queued account, and no use
    /// of a connected one. What a dial finds to dial at once after it (a shared store whose parent
    /// connected, an account a change was asked for meanwhile) is dialled this way, so one
    /// Refresh never dials an account that failed in it a second time.
    pub(crate) fn retry_only(&self, ids: &[String]) {
        let taken: Vec<String> = {
            let mut queued = lock(&self.disconnected);
            ids.iter()
                .filter(|id| queued.remove(id.as_str()))
                .cloned()
                .collect()
        };
        self.dial_from_queue(taken, false);
    }

    /// Dials `ids`, taken from the queue, with the shared stores they open; and, `with_uses`, the
    /// uses of connected accounts that did not connect or are due.
    fn dial_from_queue(&self, mut ids: Vec<String>, with_uses: bool) {
        // A shared store is opened by its parent's sign-in, so dialling the parent again dials
        // it again too: otherwise it keeps a connection made with what the parent had before.
        let derived: HashSet<String> = ids
            .iter()
            .flat_map(|id| self.registry.shared_children(id))
            .map(|child| child.id.as_str().to_owned())
            .filter(|child| !ids.contains(child))
            .collect();
        ids.extend(derived.iter().cloned());
        // A parent's dial replaces the connections its shared stores draw on, so a shared
        // store whose parent is dialled now is dialled once that dial is over, on what it made,
        // if the dial connected the parent; otherwise it waits with the parent
        // ([`Claim::release`]).
        let dialled: HashSet<String> = ids.iter().cloned().collect();
        let mut claim = Claim {
            redialling: Arc::clone(&self.redialling),
            disconnected: Arc::clone(&self.disconnected),
            ids: Vec::new(),
            after_parent: Vec::new(),
        };
        ids.retain(|id| {
            match self
                .registry
                .shared_parent(id)
                .filter(|(parent, _)| dialled.contains(parent))
            {
                Some((parent, _)) => {
                    claim.after_parent.push(Waiting {
                        store: id.clone(),
                        parent,
                        from_queue: !derived.contains(id),
                    });
                    false
                }
                None => true,
            }
        });
        let queued: HashSet<String> = ids.iter().cloned().collect();
        // An account whose mail is up but whose calendar or address book could not be reached has
        // those dialled again: nothing else would, until the next launch.
        // So is one whose calendar and contacts connected more than a day ago: their next connect
        // starts from the principal again, so a home the server moved is followed within a day.
        let uses: Vec<String> = if with_uses {
            let due = self.registry.dav_due(std::time::Instant::now());
            self.app
                .accounts_with_use_problem(mailcal_app::UseProblem::Unreachable)
                .into_keys()
                // One reached only over plaintext is dialled again too, badging nothing, in case
                // its server or its address changed.
                .chain(self.app.accounts_with_insecure_use())
                .chain(due)
                // A refresh widened its grant to open a use the dial before it was planned
                // without.
                .chain(self.registry.take_widened())
                .collect()
        } else {
            Vec::new()
        };
        for id in uses {
            if !queued.contains(&id) && !ids.contains(&id) {
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
        claim.ids.clone_from(&ids);
        // Snapshot each account's reconnection plan under the registry lock (so no lock is held
        // across the re-dial); an id whose registry entry is gone (removed meanwhile) drops out.
        // The catch-up sync after a successful reconnect resolves the account's own depth.
        let (whole, uses): (Vec<_>, Vec<_>) = ids
            .into_iter()
            .filter_map(|id| {
                let account_id = AccountId::try_from(id.as_str()).ok()?;
                let plan: account_registry::AccountDial = self.registry.dial(&id)?;
                Some((queued.contains(&id), (account_id, plan)))
            })
            .partition(|(disconnected, _)| *disconnected);
        let whole: Vec<_> = whole.into_iter().map(|(_, plan)| plan).collect();
        let uses: Vec<_> = uses.into_iter().map(|(_, plan)| plan).collect();
        if whole.is_empty() && uses.is_empty() {
            return;
        }
        log::info!(
            "retry_connections: re-dialling {} account(s), and the calendar and contacts of {} \
             more",
            whole.len(),
            uses.len(),
        );
        let reconnect = reconnect::reconnect_all(
            Arc::clone(&self.app),
            Arc::clone(&self.background),
            Arc::clone(&self.registry),
            Arc::clone(&self.disconnected),
            Arc::clone(&self.redialling),
            whole,
            self.device_zone.clone(),
        );
        let reconnect_uses = reconnect::reconnect_uses(
            Arc::clone(&self.app),
            Arc::clone(&self.registry),
            Arc::clone(&self.redialling),
            uses,
            self.device_zone.clone(),
        );
        let app = self.rediscovery.app();
        self.runtime.spawn(async move {
            // One after the other, so a retry never dials more than `MAX_CONCURRENT_DIALS` at
            // once; the calendar and contacts first, being a few requests each.
            let mut unanswered = reconnect_uses.await;
            let reconnected = reconnect.await;
            unanswered.extend(reconnected.unanswered.iter().cloned());
            let dial_again = claim.release(Some(&reconnected));
            // Off the runtime: looking again blocks on discovery and on the move's connect, and
            // the last handle on the app must not be dropped on a thread of its own runtime.
            if let Some(app) = app {
                std::thread::spawn(move || {
                    app.settle_notice_retries();
                    if !dial_again.is_empty() {
                        app.retry_only(&dial_again);
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

    use super::{Claim, Reconnected};

    fn claim_of(
        redialling: &super::Redialling,
        ids: &[&str],
    ) -> (Claim, Arc<Mutex<HashSet<String>>>) {
        let disconnected = Arc::new(Mutex::new(HashSet::new()));
        let claim = Claim {
            redialling: Arc::clone(redialling),
            disconnected: Arc::clone(&disconnected),
            ids: ids.iter().map(|&id| id.to_owned()).collect(),
            after_parent: Vec::new(),
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
        assert_eq!(
            claim.release(Some(&dialled(&[], &[]))),
            ["asked".to_owned()],
            "one id was asked for again"
        );
        assert!(redialling.lock().unwrap().is_empty());
        assert_eq!(
            *disconnected.lock().unwrap(),
            HashSet::from(["asked".to_owned()]),
            "only the id asked for again is queued, so a working account is not dialled twice"
        );
        assert!(
            claim.release(Some(&dialled(&[], &[]))).is_empty(),
            "a released claim gives nothing back twice"
        );
    }

    /// A claim holding `store`, waiting for `parent`'s dial; `from_queue` says whether it was
    /// taken from the queue or derived from its parent.
    fn waiting(from_queue: bool) -> (Claim, Arc<Mutex<HashSet<String>>>) {
        let (mut claim, disconnected) = claim_of(&Arc::default(), &[]);
        claim.after_parent = vec![super::Waiting {
            store: "store".to_owned(),
            parent: "parent".to_owned(),
            from_queue,
        }];
        (claim, disconnected)
    }

    /// What the parent's dial did, as `reconnect_all` reports it.
    fn dialled(connected: &[&str], requeued: &[&str]) -> Reconnected {
        let ids = |ids: &[&str]| ids.iter().map(|&id| id.to_owned()).collect();
        Reconnected {
            unanswered: Vec::new(),
            connected: ids(connected),
            requeued: ids(requeued),
        }
    }

    /// A shared store is dialled at once after a dial that connected its parent, waits with a
    /// parent the dial queued again, and is dropped with any other outcome, such as a refused
    /// sign-in, which waits for a new credential. When the dial never ran, one taken from the
    /// queue keeps its place, unless another pass dials its parent, and one derived from its
    /// parent is the parent's next dial's.
    #[test]
    fn a_store_waiting_for_its_parent_follows_what_the_parents_dial_did() {
        let store = HashSet::from(["store".to_owned()]);
        let queue_after = |from_queue, outcome: Option<Reconnected>| {
            let (mut claim, queue) = waiting(from_queue);
            let again = claim.release(outcome.as_ref());
            let queued = queue.lock().unwrap().clone();
            (again, queued)
        };

        let (again, queued) = queue_after(true, Some(dialled(&["parent"], &[])));
        assert_eq!(again, ["store".to_owned()], "dialled at once");
        assert_eq!(queued, store);

        let (again, queued) = queue_after(false, Some(dialled(&[], &["parent"])));
        assert!(again.is_empty());
        assert_eq!(queued, store, "waits with its parent");

        // Refused: neither connected nor queued again, whatever else queued the parent meanwhile.
        for from_queue in [true, false] {
            let (mut refused, queue) = waiting(from_queue);
            queue.lock().unwrap().insert("parent".to_owned());
            assert!(refused.release(Some(&dialled(&[], &[]))).is_empty());
            assert!(
                !queue.lock().unwrap().contains("store"),
                "waits for a new credential"
            );
        }

        let (again, queued) = queue_after(true, None);
        assert!(again.is_empty());
        assert_eq!(queued, store, "taken from the queue, it keeps its place");
        let (_, queued) = queue_after(false, None);
        assert!(queued.is_empty(), "derived, it is the parent's next dial's");

        let (mut racing, queue) = waiting(true);
        racing
            .redialling
            .lock()
            .unwrap()
            .insert("parent".to_owned(), true);
        assert!(racing.release(None).is_empty());
        assert!(
            queue.lock().unwrap().is_empty(),
            "taken from the queue while another pass dials its parent, it is that dial's"
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
