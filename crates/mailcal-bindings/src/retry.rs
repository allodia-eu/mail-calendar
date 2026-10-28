//! Dialling again what a launch, or an earlier retry, could not reach.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use engine_api::AccountId;

use crate::{MailcalApp, account_registry, reconnect};

/// The ids a retry is dialling, released from `redialling` when the dial is over, however it
/// ends: an id left claimed would never be dialled again.
struct Claim {
    redialling: Arc<Mutex<HashSet<String>>>,
    ids: Vec<String>,
}

impl Drop for Claim {
    fn drop(&mut self) {
        // Released even while unwinding: a poisoned set still holds claims to give back.
        let mut redialling = self
            .redialling
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for id in &self.ids {
            redialling.remove(id);
        }
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
    /// twice: one drained from `disconnected` is put back, so the dial it asked for, planned from
    /// what the account is now, happens at the next retry rather than not at all.
    /// Fire-and-forget on the runtime, so a slow re-dial never blocks the host's `dispatch`. A
    /// no-op when there is nothing to dial.
    pub(crate) fn retry_connections(&self) {
        let mut ids: Vec<String> = self
            .disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .drain()
            .collect();
        // A shared mailbox is opened by its parent's sign-in, so dialling the parent again dials
        // it again too: otherwise it keeps a connection made with what the parent had before.
        let shared: Vec<String> = ids
            .iter()
            .flat_map(|id| self.registry.shared_mailboxes(id))
            .map(|(child, _)| child.as_str().to_owned())
            .filter(|child| !ids.contains(child))
            .collect();
        ids.extend(shared);
        // A parent's dial replaces the connections its shared mailboxes draw on, so a shared
        // mailbox whose parent is dialled now is dialled once that dial is over, on what it made.
        let dialled: HashSet<String> = ids.iter().cloned().collect();
        let (after_parent, now): (Vec<String>, Vec<String>) = ids.into_iter().partition(|id| {
            self.registry
                .shared_parent(id)
                .is_some_and(|(parent, _)| dialled.contains(&parent))
        });
        ids = now;
        let queued: HashSet<String> = ids.iter().cloned().collect();
        // An account whose mail is up but whose calendar or address book could not be reached has
        // those dialled again: nothing else would, until the next launch.
        // So is one whose calendar and contacts connected more than a day ago: their next connect
        // starts from the principal again, so a home the server moved is followed within a day.
        let due = self.registry.dav_due(std::time::Instant::now());
        for id in self
            .app
            .accounts_with_use_problem(mailcal_app::UseProblem::Unreachable)
            .into_keys()
            .chain(due)
        {
            if !queued.contains(&id) && !ids.contains(&id) {
                ids.push(id);
            }
        }
        // An account already being dialled is left to that dial. One that was queued is queued
        // again, since the dial under way was planned before it was.
        let busy: Vec<String> = {
            let mut redialling = self.redialling.lock().expect("redialling mutex poisoned");
            let (claimed, busy) = ids
                .into_iter()
                .partition::<Vec<_>, _>(|id| redialling.insert(id.clone()));
            ids = claimed;
            busy
        };
        self.disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .extend(busy.into_iter().filter(|id| queued.contains(id)));
        if ids.is_empty() {
            return;
        }
        let claim = Claim {
            redialling: Arc::clone(&self.redialling),
            ids: ids.clone(),
        };
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
            whole,
            self.device_zone.clone(),
        );
        let reconnect_uses = reconnect::reconnect_uses(
            Arc::clone(&self.app),
            Arc::clone(&self.registry),
            uses,
            self.device_zone.clone(),
        );
        let app = self.rediscovery.app();
        self.runtime.spawn(async move {
            // One after the other, so a retry never dials more than `MAX_CONCURRENT_DIALS` at
            // once; the calendar and contacts first, being a few requests each.
            let mut unanswered = reconnect_uses.await;
            unanswered.extend(reconnect.await);
            drop(claim);
            if let Some(app) = app.as_ref().filter(|_| !after_parent.is_empty()) {
                app.disconnected
                    .lock()
                    .expect("disconnected mutex poisoned")
                    .extend(after_parent);
                app.retry_connections();
            }
            // Off the runtime: looking again blocks on discovery and on the move's connect.
            if let Some(app) = app.filter(|_| !unanswered.is_empty()) {
                std::thread::spawn(move || {
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
        collections::HashSet,
        sync::{Arc, Mutex},
    };

    use super::Claim;

    #[test]
    fn a_claim_gives_its_ids_back_even_when_its_dial_panics() {
        let redialling = Arc::new(Mutex::new(HashSet::from(["a".to_owned(), "b".to_owned()])));
        let claim = Claim {
            redialling: Arc::clone(&redialling),
            ids: vec!["a".to_owned()],
        };
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _held = claim;
            panic!("the dial failed");
        }));
        assert!(unwound.is_err());
        assert_eq!(
            *redialling.lock().unwrap(),
            HashSet::from(["b".to_owned()]),
            "only the claimed id is released"
        );
    }
}
