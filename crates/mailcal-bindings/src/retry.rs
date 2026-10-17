//! Dialling again what a launch, or an earlier retry, could not reach.

use std::sync::Arc;

use engine_api::AccountId;

use crate::{MailcalApp, account_registry, reconnect};

impl MailcalApp {
    /// Re-runs the full connect for every account sitting **disconnected** (a boot outage kept
    /// as a placeholder, or a prior retry that failed) and joins each that succeeds back into the
    /// app with live providers: so a recovered provider heals without an app restart, regaining
    /// every folder, its capabilities, and its calendar (not a degraded INBOX-only state). The
    /// disconnected set is drained optimistically so a concurrent trigger (a Refresh racing a
    /// return-to-online) never dials an account twice; a still-unreachable account is re-queued
    /// for the next attempt, and one whose sign-in was refused waits for a replaced credential.
    /// Fire-and-forget on the runtime, so a slow re-dial never blocks the host's `dispatch`. A
    /// no-op when nothing is disconnected.
    pub(crate) fn retry_connections(&self) {
        let mut ids: Vec<String> = self
            .disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .drain()
            .collect();
        // An account whose mail is up but whose calendar or address book could not be reached is
        // dialled again too: nothing else would, until the next launch.
        for id in self
            .app
            .accounts_with_use_problem(mailcal_app::UseProblem::Unreachable)
            .into_keys()
        {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        // An account already being dialled again is left to that dial; one that fails again is
        // back in `disconnected`, or still has its unreachable use, for the next retry.
        {
            let mut redialling = self.redialling.lock().expect("redialling mutex poisoned");
            ids.retain(|id| redialling.insert(id.clone()));
        }
        if ids.is_empty() {
            return;
        }
        let claimed = ids.clone();
        let release = {
            let redialling = Arc::clone(&self.redialling);
            move || {
                let mut redialling = redialling.lock().expect("redialling mutex poisoned");
                for id in &claimed {
                    redialling.remove(id);
                }
            }
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
            release();
            return;
        }
        log::info!(
            "retry_connections: re-dialing {} account(s) with something unreachable",
            plans.len(),
        );
        let reconnect = reconnect::reconnect_all(
            Arc::clone(&self.app),
            Arc::clone(&self.background),
            Arc::clone(&self.registry),
            Arc::clone(&self.disconnected),
            plans,
            self.device_zone.clone(),
        );
        let app = self.rediscovery.app();
        self.runtime.spawn(async move {
            let unanswered = reconnect.await;
            release();
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
