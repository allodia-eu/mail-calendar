//! Sync-progress aggregation: the bar for a download the user awaits, the hint for one they did
//! not ask for.
//!
//! The engine owns per-scope commit aggregation via [`AccountProgress`] and reports each pass's
//! folders through [`SyncObserver`]. This module keeps only the app policy around it: which pass
//! is allowed to raise which surface, several passes at once, and signalling
//! [`Surface::SyncProgress`] when either moves.
//!
//! The split the two surfaces hold to is in [`SyncProgressSnapshot`]: the bar belongs to a pass
//! the user started and is waiting on; a background pass never raises it, and instead names
//! itself in the hint, but only once it has actually committed mail, so a poll that finds
//! nothing stays silent.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use engine_api::{AccountId, AccountProgress, Provider, SyncCommit, SyncObserver, SyncScope};
use engine_core::ids::MailboxId;
use mailcal_viewmodel::{AccountSyncProgress, SyncProgressSnapshot};

use crate::{App, Surface, sync_progress_staged::pretended_progress};

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
}

impl SyncProgressState {
    fn begin(
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

    fn end(&mut self, id: u64) {
        if let Some(pass) = self.passes.remove(&id) {
            pass.progress.finish();
        }
    }

    /// Registers an account and how many folders its pass set out to sync.
    fn account_started(&mut self, id: u64, account: &str, folders: u32) {
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
    fn folder_finished(&mut self, id: u64, account: &str) -> bool {
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
    fn account_finished(&mut self, id: u64, account: &str) -> bool {
        let Some(pass) = self.passes.get_mut(&id) else {
            return false;
        };
        pass.accounts
            .remove(account)
            .is_some_and(|entry| entry.downloading)
    }

    /// Admits an account to the hint: this pass has committed mail for it. Returns whether that
    /// changed anything, so the per-message commit path signals once rather than every time.
    fn downloading(&mut self, id: u64, account: &str) -> bool {
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
    fn warming(&mut self, account: &str, done: Option<u32>) -> bool {
        match done {
            Some(done) => self.warming.insert(account.to_owned(), done) != Some(done),
            None => self.warming.remove(account).is_some(),
        }
    }

    fn snapshot(&self) -> SyncProgressSnapshot {
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
        }
    }

    /// The accounts currently catching up in the background, summed across passes; two of them
    /// (a poll tick and a push refresh, say) can be syncing the same account at once, and the
    /// hint counts folders, not passes.
    ///
    /// A body warm is merged in as the same account's second phase. An account with a pass still
    /// running keeps its folder counts: the folders are what it is waiting on, and a warm left
    /// over from the pass before would otherwise overwrite them.
    fn hint(&self) -> Vec<AccountSyncProgress> {
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
        hinted.into_values().collect()
    }
}

/// A [`SyncObserver`] that folds one pass's commits into an engine [`AccountProgress`], tracks
/// its accounts' folders, and signals the host to re-read the progress surface.
pub(crate) struct ProgressForwarder<'a, P: Provider> {
    id: u64,
    label: &'static str,
    progress: Arc<AccountProgress>,
    total_logged: AtomicBool,
    /// Commits this pass took, and how many of them republished the list. What a streamed pass
    /// costs on this side of the engine is the second number, not the first; each republish
    /// re-projects the cached window. Reported once, at the end of the pass.
    commits: AtomicU64,
    republished: AtomicU64,
    app: &'a App<P>,
}

impl<P: Provider> SyncObserver for ProgressForwarder<'_, P> {
    fn committed(&self, commit: &SyncCommit<'_>) {
        self.progress.committed(commit);
        let snap = self.progress.snapshot();
        if let Some(total) = snap.total
            && self
                .total_logged
                .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            log::info!("{}: download total known: {total} message(s)", self.label);
        }
        if !commit.upserted.is_empty() {
            self.app
                .note_download(self.id, commit.scope.account().as_str());
        }
        // Publishing the rebuilt list is what signals it; the bool only says whether it moved.
        self.commits.fetch_add(1, Ordering::Relaxed);
        if self.app.apply_live_mailbox_commit(commit) {
            self.republished.fetch_add(1, Ordering::Relaxed);
        }
        self.app.observer.surface_changed(Surface::SyncProgress);
    }

    /// Records the account's Inbox before its folders stream, and how many folders this pass has
    /// to get through.
    ///
    /// The unified view keeps a live-spliced row only when its folder is the account's inbox
    /// ([`App::live_inbox_keys`]), so this has to be known *before* the rows arrive: the pass
    /// reports it here for exactly that reason. Learning it when the pass ended would leave a
    /// freshly added account's mail invisible while it downloaded, which is the one time a user
    /// is watching the list fill.
    fn account_sync_started(&self, account: &AccountId, folders: usize, inbox: Option<&MailboxId>) {
        self.app.note_account_started(self.id, account, folders);
        let Some(inbox) = inbox else {
            return;
        };
        self.app
            .inbox_keys
            .lock()
            .expect("inbox-key mutex poisoned")
            .insert(account.as_str().to_owned(), inbox.key().as_str().to_owned());
    }

    fn folder_sync_finished(&self, account: &AccountId, _scope: &SyncScope, _synced: bool) {
        self.app.note_folder_finished(self.id, account);
    }

    fn account_sync_finished(&self, account: &AccountId) {
        self.app.note_account_finished(self.id, account);
    }
}

impl<P: Provider> App<P> {
    /// The current sync-progress snapshot (pulled after a [`Surface::SyncProgress`] signal).
    #[must_use]
    pub fn sync_progress(&self) -> SyncProgressSnapshot {
        if let Some(staged) = pretended_progress() {
            return staged;
        }
        self.sync_progress
            .lock()
            .expect("sync-progress mutex poisoned")
            .snapshot()
    }

    /// Marks a sync as started with a log label for the first known progress denominator.
    ///
    /// `awaited` is the bar: a download the user started and is waiting on (adding an account,
    /// opening an unsynced folder, an explicit refetch) shows it from the start. A background
    /// pass never raises it; it would take a row of layout for work the user did not ask for,
    /// and the same information fits in the status line the footer already draws.
    ///
    /// `announceable` is whether a background pass may reach that status line at all. A pass that
    /// follows the user's **own** mail action passes `false`: archiving a message re-commits it
    /// (it moved folders), so every action would otherwise announce a sync the user neither
    /// started nor waits on. The row already left the list optimistically; there is nothing to
    /// explain.
    pub(crate) fn begin_sync_labeled(
        &self,
        awaited: bool,
        announceable: bool,
        scopes: usize,
        label: &'static str,
    ) -> ProgressForwarder<'_, P> {
        let (id, progress) = self
            .sync_progress
            .lock()
            .expect("sync-progress mutex poisoned")
            .begin(awaited, announceable, scopes);
        self.observer.surface_changed(Surface::SyncProgress);
        ProgressForwarder {
            id,
            label,
            progress,
            total_logged: AtomicBool::new(false),
            commits: AtomicU64::new(0),
            republished: AtomicU64::new(0),
            app: self,
        }
    }

    /// Marks a sync as finished and signals the progress surface.
    pub(crate) fn end_sync(&self, progress: &ProgressForwarder<'_, P>) {
        let commits = progress.commits.load(Ordering::Relaxed);
        if commits > 0 {
            log::info!(
                "{}: {commits} commit(s) rebuilt the list {} time(s)",
                progress.label,
                progress.republished.load(Ordering::Relaxed),
            );
        }
        self.sync_progress
            .lock()
            .expect("sync-progress mutex poisoned")
            .end(progress.id);
        self.observer.surface_changed(Surface::SyncProgress);
    }

    fn note_account_started(&self, id: u64, account: &AccountId, folders: usize) {
        self.sync_progress
            .lock()
            .expect("sync-progress mutex poisoned")
            .account_started(
                id,
                account.as_str(),
                u32::try_from(folders).unwrap_or(u32::MAX),
            );
    }

    fn note_folder_finished(&self, id: u64, account: &AccountId) {
        if self
            .sync_progress
            .lock()
            .expect("sync-progress mutex poisoned")
            .folder_finished(id, account.as_str())
        {
            self.observer.surface_changed(Surface::SyncProgress);
        }
    }

    fn note_account_finished(&self, id: u64, account: &AccountId) {
        if self
            .sync_progress
            .lock()
            .expect("sync-progress mutex poisoned")
            .account_finished(id, account.as_str())
        {
            self.observer.surface_changed(Surface::SyncProgress);
        }
    }

    /// Admits an account to the background hint once its pass has downloaded mail. The commit
    /// path already signals the surface, so this only records.
    fn note_download(&self, id: u64, account: &str) {
        self.sync_progress
            .lock()
            .expect("sync-progress mutex poisoned")
            .downloading(id, account);
    }

    /// Reports an account's body warm to the hint: `Some(done)` while it runs, `None` when it
    /// ends. Signals only when the hint actually moved, so a pass with nothing to warm; the
    /// steady state; stays silent.
    pub(crate) fn note_warming(&self, account: &AccountId, done: Option<u32>) {
        if self
            .sync_progress
            .lock()
            .expect("sync-progress mutex poisoned")
            .warming(account.as_str(), done)
        {
            self.observer.surface_changed(Surface::SyncProgress);
        }
    }
}

#[cfg(test)]
#[path = "sync_progress_tests.rs"]
mod progress_tests;
