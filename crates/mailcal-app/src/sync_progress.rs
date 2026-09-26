//! Sync progress, app side: the observer a pass reports through, and the entry points that
//! raise and clear what a host draws.
//!
//! The engine owns per-scope commit aggregation via [`AccountProgress`] and reports each pass's
//! folders through [`SyncObserver`]. This module keeps only the app policy around it: which pass
//! is allowed to raise which surface, several passes at once, and signalling
//! [`Surface::SyncProgress`] when any of them moves. The bookkeeping underneath is
//! [`SyncProgressState`](crate::sync_progress_state::SyncProgressState).
//!
//! The split the three surfaces hold to is in [`SyncProgressSnapshot`]: the bar belongs to a
//! pass the user started and is waiting on; a background pass never raises it, and instead names
//! itself in the hint, but only once it has actually committed mail, so a poll that finds
//! nothing stays silent; and an account whose server asked it to wait says so in the hint's
//! place, because nothing is arriving for it and nothing is wrong.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use engine_api::{AccountId, AccountProgress, Provider, SyncCommit, SyncObserver, SyncScope};
use engine_core::ids::MailboxId;
use mailcal_viewmodel::SyncProgressSnapshot;

use crate::{App, Surface, sync_progress_staged::pretended_progress, sync_progress_state::Pause};

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

    /// Applies one pass's rate-limit verdict to `account`'s paused notice: `Some(true)` raises
    /// it (with `wait`, the longest instant any scope in the pass named, where one did),
    /// `Some(false)` clears it, and `None` leaves it exactly as it was, for the pass that proved
    /// nothing because every scope was already held by another one.
    ///
    /// Signals [`Surface::SyncProgress`] only when what a host would draw actually changed, so
    /// an account that polls into the same one-minute window every tick does not redraw the
    /// status line each time.
    pub(crate) fn apply_throttle(
        &self,
        account: &AccountId,
        throttled: Option<bool>,
        wait: Option<Duration>,
    ) {
        let verdict = match (throttled, wait) {
            (Some(true), Some(wait)) => Pause::Until(wait),
            (Some(true), None) => Pause::Untimed,
            (Some(false), _) => Pause::Over,
            // The pass proved nothing: every scope was already held by another one.
            (None, _) => return,
        };
        if self
            .sync_progress
            .lock()
            .expect("sync-progress mutex poisoned")
            .paused(account.as_str(), verdict)
        {
            // Counts and a reason, never the address (`docs/logging.md`). The figure is worth a
            // line of its own: a support log that says a server asked for eleven minutes
            // explains a quiet hour that otherwise reads as a broken sync.
            match verdict {
                Pause::Until(wait) => log::info!(
                    "sync: an account's server asked us to slow down; pausing its sync for \
                     {}s, as the server stated",
                    wait.as_secs(),
                ),
                Pause::Untimed => log::info!(
                    "sync: an account's server asked us to slow down without saying for how \
                     long; pausing its sync until the next scheduled pass",
                ),
                Pause::Over => {
                    log::info!("sync: an account's server is accepting traffic again");
                }
            }
            self.observer.surface_changed(Surface::SyncProgress);
        }
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
