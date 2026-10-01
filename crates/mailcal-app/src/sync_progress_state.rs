//! The aggregation behind [`SyncProgressSnapshot`]: the in-flight passes, the body warms that
//! follow them, and the accounts a server has asked to wait.
//!
//! Split out of `sync_progress.rs`, which keeps the app wiring around it, so both stay under
//! the size limit. The policy the three surfaces hold to is stated on [`SyncProgressSnapshot`];
//! what is here is the bookkeeping that produces it.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::{Duration, Instant},
};

use engine_api::AccountProgress;
use mailcal_viewmodel::{AccountSyncProgress, SyncProgressSnapshot, ThrottledAccount};

/// What one sync pass learned about an account's rate limit.
///
/// Three cases, named rather than nested, because "refused with a window", "refused without
/// one" and "not refused" are three different sentences a host draws, and about two Gmail
/// refusals in three are the middle one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pause {
    /// Refused, and the server named when the limit clears.
    Until(Duration),
    /// Refused, with no instant given.
    Untimed,
    /// Not refused: whatever notice was up comes down.
    Over,
}

/// One account's folders within a pass, as far as the hint needs them.
#[derive(Debug, Default)]
struct AccountPass {
    folders_total: u32,
    folders_done: u32,
    /// Set once this account has committed mail, which is what admits it to the hint.
    downloading: bool,
}

#[derive(Debug)]
struct SyncPass {
    /// A download the user is waiting on: the bar is up from the moment the pass starts, and
    /// this pass never appears in the hint.
    awaited: bool,
    /// Whether this pass may name itself at all once it downloads. False for the pass that
    /// follows the user's *own* mail action; see [`App::begin_sync_labeled`].
    announceable: bool,
    progress: Arc<AccountProgress>,
    /// The accounts this pass is syncing, keyed by id so the hint's order is stable across
    /// snapshots. Only tracked for a pass that could reach the hint.
    accounts: BTreeMap<String, AccountPass>,
}

impl SyncPass {
    /// Whether this pass's accounts can reach the hint, and so are worth tracking.
    fn hints(&self) -> bool {
        self.announceable && !self.awaited
    }
}

/// The app-level set of in-flight sync passes, and the body warms that follow them.
#[derive(Debug, Default)]
pub(crate) struct SyncProgressState {
    next_id: u64,
    passes: HashMap<u64, SyncPass>,
    /// Accounts warming message bodies, and how many are done. A warm is not a pass; it runs
    /// after one, drains against "what is still missing" rather than a folder list, and belongs
    /// to no observer: so it is tracked beside them and merged into the hint.
    warming: BTreeMap<String, u32>,
    /// Accounts a server has asked to wait, each with the instant it may be synced again where
    /// one was named. Stored as a **deadline** rather than a remaining wait so the figure a
    /// host renders is recomputed on every pull instead of ageing in place.
    paused: BTreeMap<String, Option<Instant>>,
}

impl SyncProgressState {
    pub(crate) fn begin(
        &mut self,
        awaited: bool,
        announceable: bool,
        scopes: usize,
    ) -> (u64, Arc<AccountProgress>) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let progress = Arc::new(AccountProgress::new(scopes));
        progress.begin();
        self.passes.insert(
            id,
            SyncPass {
                awaited,
                announceable,
                progress: Arc::clone(&progress),
                accounts: BTreeMap::new(),
            },
        );
        (id, progress)
    }

    pub(crate) fn end(&mut self, id: u64) {
        if let Some(pass) = self.passes.remove(&id) {
            pass.progress.finish();
        }
    }

    /// Registers an account and how many folders its pass set out to sync.
    pub(crate) fn account_started(&mut self, id: u64, account: &str, folders: u32) {
        let Some(pass) = self.passes.get_mut(&id).filter(|pass| pass.hints()) else {
            return;
        };
        pass.accounts.insert(
            account.to_owned(),
            AccountPass {
                folders_total: folders,
                ..AccountPass::default()
            },
        );
    }

    /// Counts one of an account's folders as done; whether it synced or failed. The hint says
    /// how far through the folder list the pass is, not how much of it worked.
    ///
    /// Returns whether the hint moved, so a quiet pass does not signal the surface per folder.
    pub(crate) fn folder_finished(&mut self, id: u64, account: &str) -> bool {
        let Some(pass) = self.passes.get_mut(&id) else {
            return false;
        };
        let Some(entry) = pass.accounts.get_mut(account) else {
            return false;
        };
        entry.folders_done = entry.folders_done.saturating_add(1);
        entry.downloading
    }

    /// Drops an account whose pass has finished, so the hint clears per account rather than
    /// waiting for the slowest one in a multi-account refresh.
    pub(crate) fn account_finished(&mut self, id: u64, account: &str) -> bool {
        let Some(pass) = self.passes.get_mut(&id) else {
            return false;
        };
        pass.accounts
            .remove(account)
            .is_some_and(|entry| entry.downloading)
    }

    /// Admits an account to the hint: this pass has committed mail for it. Returns whether that
    /// changed anything, so the per-message commit path signals once rather than every time.
    pub(crate) fn downloading(&mut self, id: u64, account: &str) -> bool {
        let Some(pass) = self.passes.get_mut(&id) else {
            return false;
        };
        let Some(entry) = pass.accounts.get_mut(account) else {
            return false;
        };
        !std::mem::replace(&mut entry.downloading, true)
    }

    /// Puts `account` in the hint's body phase, or takes it out when the warm ends.
    ///
    /// Returns whether the hint moved, so a pass with nothing to warm: the steady state, once a
    /// mailbox is cached; never signals the surface.
    pub(crate) fn warming(&mut self, account: &str, done: Option<u32>) -> bool {
        match done {
            Some(done) => self.warming.insert(account.to_owned(), done) != Some(done),
            None => self.warming.remove(account).is_some(),
        }
    }

    /// Raises or clears `account`'s paused notice, from what one pass learned.
    ///
    /// Returns whether anything a host renders actually moved, so a pass that finds the account
    /// exactly as it left it never signals the surface. Re-raising with a fresh instant does
    /// move it: the wait is the whole content of the notice.
    pub(crate) fn paused(&mut self, account: &str, pause: Pause) -> bool {
        let until = match pause {
            Pause::Until(wait) => Some(Instant::now() + wait),
            Pause::Untimed => None,
            Pause::Over => return self.paused.remove(account).is_some(),
        };
        // Compare the minutes a host would draw, not the instants: a pass that meets the same
        // one-minute window twice restates the same sentence, and signalling for that would
        // redraw the status line on every poll tick of a throttled account.
        let before = self.paused.get(account).map(|at| remaining_minutes(*at));
        self.paused.insert(account.to_owned(), until);
        before != Some(remaining_minutes(until))
    }

    /// The accounts still waiting, with the wait recomputed against the clock.
    ///
    /// An instant that has passed is dropped rather than shown as zero: the pause is over, and
    /// the pass that resumes will re-raise it if the server is still refusing. An account that
    /// was refused without being given an instant stays until a pass clears it, since there is
    /// nothing to expire.
    pub(crate) fn pauses(&self) -> Vec<ThrottledAccount> {
        self.paused
            .iter()
            .filter(|(account, _)| self.still_waiting(account))
            .map(|(account, until)| ThrottledAccount {
                account_id: account.clone(),
                resumes_in_minutes: remaining_minutes(*until),
            })
            .collect()
    }

    /// Whether `account`'s notice is up **now**.
    ///
    /// An instant that has passed takes the notice down on its own, without anything having to
    /// come back and retract it: the pass that resumes re-raises it if the server is still
    /// refusing. The entry itself stays until a pass clears it, which is what lets a refusal
    /// that named no instant persist, since it has nothing to expire.
    fn still_waiting(&self, account: &str) -> bool {
        match self.paused.get(account) {
            Some(Some(until)) => *until > Instant::now(),
            Some(None) => true,
            None => false,
        }
    }

    pub(crate) fn snapshot(&self) -> SyncProgressSnapshot {
        let awaited: Vec<_> = self.passes.values().filter(|pass| pass.awaited).collect();
        let active = !awaited.is_empty();
        let mut fetched = 0_u64;
        let mut total = Some(0_u64);
        for pass in awaited {
            let snap = pass.progress.snapshot();
            fetched += snap.fetched as u64;
            total = match (total, snap.total) {
                (Some(acc), Some(next)) => Some(acc + next as u64),
                _ => None,
            };
        }
        SyncProgressSnapshot {
            active,
            fetched,
            total: active.then_some(total).flatten(),
            accounts: self.hint(),
            throttled: self.pauses(),
        }
    }

    /// The accounts currently catching up in the background, summed across passes; two of them
    /// (a poll tick and a push refresh, say) can be syncing the same account at once, and the
    /// hint counts folders, not passes.
    ///
    /// A body warm is merged in as the same account's second phase. An account with a pass still
    /// running keeps its folder counts: the folders are what it is waiting on, and a warm left
    /// over from the pass before would otherwise overwrite them.
    ///
    /// An account whose notice is up is left out entirely. The two surfaces do not overlap, and
    /// resolving that in five clients rather than here is how they come to disagree: the one
    /// case where both are live at once is the pass that resumes a refusal that named no
    /// instant, and it must not read as "syncing" while the notice beside it says otherwise.
    pub(crate) fn hint(&self) -> Vec<AccountSyncProgress> {
        let mut hinted: BTreeMap<&str, AccountSyncProgress> = BTreeMap::new();
        for pass in self.passes.values().filter(|pass| pass.hints()) {
            for (account, entry) in pass.accounts.iter().filter(|(_, e)| e.downloading) {
                let row = hinted
                    .entry(account.as_str())
                    .or_insert_with(|| AccountSyncProgress {
                        account_id: account.clone(),
                        ..AccountSyncProgress::default()
                    });
                row.folders_done = row.folders_done.saturating_add(entry.folders_done);
                row.folders_total = row.folders_total.saturating_add(entry.folders_total);
            }
        }
        for (account, done) in &self.warming {
            let row = hinted
                .entry(account.as_str())
                .or_insert_with(|| AccountSyncProgress {
                    account_id: account.clone(),
                    ..AccountSyncProgress::default()
                });
            if row.folders_total == 0 {
                row.warming_bodies = true;
                row.bodies_done = *done;
            }
        }
        // Expiry is read here too, not merely membership: once the wait elapses the notice is
        // down, and an account that then starts downloading has to be able to say so without
        // waiting for the pass that clears the entry.
        hinted.retain(|account, _| !self.still_waiting(account));
        hinted.into_values().collect()
    }
}

/// Whole minutes from now until `until`, rounded **up** and never zero, or `None` when no
/// instant was named.
///
/// Up, because a wait understated is a promise the next pass breaks, while one overstated is
/// merely a sync that arrives early. Never zero for the same reason: an instant a fraction of a
/// second away is still a wait, and "in about 0 minutes" is not a sentence.
fn remaining_minutes(until: Option<Instant>) -> Option<u32> {
    let left = until?.saturating_duration_since(Instant::now());
    // Ceiling on milliseconds, not seconds: a wait of 61s is already 60.999s by the time it is
    // measured, and truncating to whole seconds first would round it down to one minute, which
    // is the opposite of what the rule above asks for.
    let minutes = left.as_millis().div_ceil(60_000).max(1);
    Some(u32::try_from(minutes).unwrap_or(u32::MAX))
}

#[cfg(test)]
#[path = "sync_progress_tests.rs"]
mod progress_tests;
