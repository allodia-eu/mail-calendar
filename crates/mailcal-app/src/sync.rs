//! Background mail-sync orchestration: the app methods that reconcile + sync whole **accounts**.
//!
//! The app drives each account's folders concurrently: the folder list once, then every
//! folder's email streamed in parallel (`engine-api`'s per-folder split): so the work overlaps
//! instead of running one folder at a time on a single core. The per-account sync itself and its
//! progress aggregation live in `sync_account` and `sync_progress`; the targeted one-folder
//! refresh (a push, or the folder you just opened) lives in `sync_folder`.

use std::time::{Duration, Instant};

use engine_api::{AccountId, CalendarDate, Provider, StreamTuning, SyncWindow};
use futures::future::join_all;
use mailcal_account::SyncDepth;

use crate::{
    App,
    sync_account::{SyncAccountOutcome, sync_account_providers},
};

/// Objects a streamed pass asks the provider for per network round trip.
const STREAM_FETCH_BATCH: usize = 200;

/// Objects committed per chunk, and so the granularity a pass costs, not just reports.
///
/// Each commit splices the cached rows and rebuilds the visible list from them
/// (`live_mailbox.rs`): a clone of the whole cached window, a scan of it per upserted message,
/// a re-sort. One message per chunk made a first sync pay that per message: a measured
/// five-account sync of 7,107 messages saturated a core for three minutes with the reading pane
/// parked behind the backlog. It is a store transaction and a thread derivation per chunk too.
///
/// A maximum, not a target: a push delivering two messages still commits once. Its ceiling is
/// how coarsely a cold list may fill in: a chunk at a time, each a fraction of the
/// [`STREAM_FETCH_BATCH`] the network already made the user wait for.
pub(crate) const STREAM_CHUNK_SIZE: usize = 50;
const ACCOUNT_SYNC_BUSY_RETRY_ATTEMPTS: usize = 30;
const ACCOUNT_SYNC_BUSY_RETRY_MS: [u64; 5] = [250, 500, 1_000, 2_000, 5_000];

/// What a [`refresh_mail`](App::refresh_mail) pass is allowed to say about its downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefreshProgress {
    /// An explicit, user-**awaited** re-download (a reset / full refetch): the bar is up from
    /// the start, even though the pre-reset list is still painted.
    Awaited,
    /// A routine background pass: no bar, and the accounts it is syncing reach the status hint
    /// only once streamed commits actually download messages.
    Background,
}

impl<P: Provider> App<P> {
    /// Reconciles + syncs every account **concurrently**, then rebuilds + signals the
    /// snapshot. Routine refreshes keep cursors intact so QRESYNC / resume can do cheap
    /// incremental work; explicit resnapshots clear cursors at their call site.
    pub(crate) async fn refresh_mail(&self, progress_policy: RefreshProgress) {
        // Offline: don't attempt the network round-trips. Over many folders and accounts they
        // would fail instantly and storm a dead network (the overnight-wake bug); the offline
        // banner already explains the pause, and coming back online re-triggers this. A host
        // that never reports reachability is always "online", so this is a no-op there.
        //
        // We still re-signal the mailbox list so the already-built (primed / cached) snapshot
        // renders: a cold offline launch's boot-prime signal can fire before the host's observer
        // is wired, and without this the list would come up blank until the device returns online
        // (the bug the Android reload-at-connect worked around, and which macOS otherwise hit).
        if !self.is_online() {
            self.mailbox_list.resignal();
            return;
        }
        // Clone the account handles, then sync over them with the read guard released; the
        // per-folder syncs are network round-trips and must not hold the lock (a long
        // round-trip would stall a concurrent `add_account`).
        let accounts = self.account_handles().await;
        log::info!("refresh_mail: syncing {} account(s)", accounts.len());
        let sync_start = Instant::now();
        // A background refresh over already-rendered cached mail raises no bar; it names the
        // accounts it is downloading for in the status hint instead. A cold load with nothing on
        // screen is a different thing: the user is waiting on the first paint, so it is awaited,
        // like an explicit re-download. The same flags pair `begin`/`end` so the in-flight pass
        // counts stay balanced.
        let (awaited, announceable) = match progress_policy {
            RefreshProgress::Awaited => (true, true),
            RefreshProgress::Background => (self.mailbox_list_is_empty(), true),
        };
        let scopes: usize = accounts.iter().map(|account| account.providers.len()).sum();
        let progress = self.begin_sync_labeled(awaited, announceable, scopes, "refresh-mail");
        // Time each account's own pass, not the whole join: the accounts sync concurrently, so
        // the pass duration is the slowest one and attributing it to every account would report
        // a fast account as slow.
        let outcomes: Vec<(SyncAccountOutcome, u64)> =
            join_all(accounts.iter().enumerate().map(|(i, account)| {
                let tuning = self.sync_tuning_for(&account.id);
                // Borrow the shared forwarder: `async move` would otherwise move it into the
                // first future, and every account's sync reports progress through the same one.
                let progress = &progress;
                async move {
                    let started = Instant::now();
                    let outcome =
                        sync_account_providers(&self.engine, account, tuning, progress, i).await;
                    (
                        outcome,
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    )
                }
            }))
            .await;
        self.end_sync(&progress);
        // Badge each account by whether its sync reached the server (an indeterminate `None`
        // leaves the badge as-is), and drop its cache (per-account) so the rebuild reloads only
        // what changed; the pass rebuilds once below.
        for (account, (outcome, elapsed_ms)) in accounts.iter().zip(outcomes) {
            if let Some(reachable) = outcome.reachable {
                self.set_account_reachable(&account.id, reachable);
                self.track_sync(&account.id, reachable, elapsed_ms);
            }
            self.apply_signin_expired(&account.id, outcome.signin_expired);
            self.invalidate_list_cache();
        }
        let sync_ms = sync_start.elapsed().as_millis();
        let rebuild_start = Instant::now();
        self.rebuild_snapshot().await;
        log::info!(
            "refresh_mail: sync {sync_ms}ms + rebuild {}ms",
            rebuild_start.elapsed().as_millis(),
        );
        // Warm every account's body cache after the list is on screen (this method runs off
        // the UI thread and the rebuild above already published the snapshot), so each synced
        // window becomes instantly openable and readable offline. Concurrent across accounts;
        // each drains on its own mail connection, and a no-op per already-warm account.
        join_all(
            accounts
                .iter()
                .map(|account| self.prefetch_account_bodies(&account.id)),
        )
        .await;
    }

    /// Syncs one account's mail (used when it is first added), without touching the others.
    /// **Streams** the result through engine commit events. The caller rebuilds afterwards for
    /// the authoritative, fully-threaded snapshot.
    pub(crate) async fn sync_account(&self, id: &AccountId) {
        let Some(account) = self.account_handle(id).await else {
            return;
        };
        // Adding an account is an explicit, user-awaited download; show the bar even if other
        // accounts' mail is already on screen.
        let progress = self.begin_sync_labeled(true, true, account.providers.len(), "account-add");
        let tuning = self.sync_tuning_for(id);
        let acct = self.account_ordinal(id).await;
        let outcome = sync_account_providers(&self.engine, &account, tuning, &progress, acct).await;
        self.end_sync(&progress);
        if let Some(reachable) = outcome.reachable {
            self.set_account_reachable(id, reachable);
        }
        self.apply_signin_expired(id, outcome.signin_expired);
        self.invalidate_list_cache();
    }

    /// Syncs a just-added account's mail with the download bar **visible**; adding an account
    /// is an explicit, user-awaited first download, so it shows progress immediately; then
    /// rebuilds the snapshot so its mail appears. The bindings register the
    /// account first (so the switcher shows it and the setup modal dismisses at once), then spawn
    /// this in the background. The deferred counterpart is
    /// [`refresh_account`](Self::refresh_account): it starts hidden, then shows progress only if
    /// it actually downloads mail. A no-op for an unknown account id.
    pub async fn sync_added_account(&self, id: &AccountId) {
        self.sync_account(id).await;
        self.rebuild_snapshot().await;
        // Fetch the account's calendar too, for the same reason the bodies below are fetched: the
        // user asked for this account, not for its mail. Without it a brand-new account has no
        // diary until the calendar tab is opened, so the first session: the one where an
        // invitation is most likely to be read; could only answer "we have not looked".
        self.refresh_calendar_in_background().await;
        // Warm the body cache for the just-synced window in the background (the bindings already
        // run this method off the UI thread): opening the account's recent mail is then instant
        // and works offline, instead of each first open blocking on (or failing without) a
        // provider fetch. Runs after the list is on screen, so it never delays the first paint.
        self.prefetch_account_bodies(id).await;
    }

    /// A background refresh of one account's eager folders, then a snapshot rebuild; the
    /// per-account polling timer's per-tick work. Unlike `sync_account` (which shows the bar for
    /// an explicit add), this starts hidden; a periodic poll over already-rendered mail does not
    /// flash a bar unless it actually downloads messages. A no-op for an unknown account id.
    pub async fn refresh_account(&self, id: &AccountId) {
        let _ = self.refresh_account_once(id, "account-refresh", true).await;
        // Every poll tick tops the body cache up (new mail, and any backlog an earlier
        // interrupted pass left), so the synced window converges on fully-warm. A cheap no-op
        // once it is (one key scan), and single-flight if a pass is already draining.
        self.prefetch_account_bodies(id).await;
    }

    /// The follow-up to the user's **own** mail action, on the one account the edit reached.
    ///
    /// The others cannot have changed because of it (the edit never left this account) so
    /// asking their servers is a round trip apiece bought by one swipe. What arrives on them
    /// meanwhile is what their poll timers and standing IDLE watches are for. Says nothing on
    /// either progress surface: a bar or a hint over the user's own archive is a row of layout
    /// appearing under their finger; see [`begin_sync_labeled`](App::begin_sync_labeled).
    pub(crate) async fn refresh_after_write(&self, id: &AccountId) {
        let _ = self
            .refresh_account_once(id, "write-follow-up", false)
            .await;
        self.prefetch_account_bodies(id).await;
    }

    /// A catch-up refresh after a placeholder account reconnects with live providers. It retries
    /// through transient scope contention so boot's placeholder refresh or just-started watches
    /// cannot make the resumed backfill silently give up.
    pub async fn refresh_reconnected_account(&self, id: &AccountId) {
        self.refresh_account_with_busy_retry(id, "reconnect", false)
            .await;
        self.prefetch_account_bodies(id).await;
    }

    /// Re-snapshots one account after a user changed its sync-depth window. If a poll, watch,
    /// or boot refresh already owns a folder scope, wait and retry instead of making the user
    /// pull-to-refresh later. The mail cursors are cleared immediately before every attempt so
    /// an older in-flight sync cannot leave a stale narrower-window cursor behind.
    pub(crate) async fn resync_account_after_depth_change(&self, id: &AccountId) {
        self.refresh_account_with_busy_retry(id, "sync-depth", true)
            .await;
        // A widened window re-snapshots older mail whose bodies were never warmed; warm them
        // so the newly-visible depth is as offline-readable as the rest.
        self.prefetch_account_bodies(id).await;
    }

    async fn refresh_account_with_busy_retry(
        &self,
        id: &AccountId,
        label: &'static str,
        clear_cursors: bool,
    ) {
        for attempt in 0..ACCOUNT_SYNC_BUSY_RETRY_ATTEMPTS {
            if clear_cursors {
                let _ = self.engine.clear_mail_cursors(id).await;
            }
            let Some(outcome) = self.refresh_account_once(id, label, true).await else {
                log::info!("{label}: account refresh deferred until connectivity returns");
                return;
            };
            if outcome.busy_scopes == 0 {
                return;
            }
            let delay = busy_retry_delay(attempt);
            log::info!(
                "{label}: account refresh hit {} busy scope(s); retrying in {}ms",
                outcome.busy_scopes,
                delay.as_millis(),
            );
            tokio::time::sleep(delay).await;
        }
        if clear_cursors {
            let _ = self.engine.clear_mail_cursors(id).await;
        }
        log::warn!(
            "{label}: account refresh still busy after {ACCOUNT_SYNC_BUSY_RETRY_ATTEMPTS} attempts",
        );
    }

    /// One account's pass. `announce` is what the account may say about itself while it runs:
    /// a poll or a catch-up names itself in the status hint once it downloads mail, the
    /// follow-up to the user's own edit says nothing at all.
    async fn refresh_account_once(
        &self,
        id: &AccountId,
        label: &'static str,
        announce: bool,
    ) -> Option<SyncAccountOutcome> {
        // A poll tick over a dead network would just fail every folder; skip it while offline
        // (the watch/poll loops also gate on this) and let the return-to-online refresh catch up.
        if !self.is_online() {
            return None;
        }
        let account = self.account_handle(id).await?;
        let progress = self.begin_sync_labeled(false, announce, account.providers.len(), label);
        let tuning = self.sync_tuning_for(id);
        let acct = self.account_ordinal(id).await;
        let outcome = sync_account_providers(&self.engine, &account, tuning, &progress, acct).await;
        self.end_sync(&progress);
        if let Some(reachable) = outcome.reachable {
            self.set_account_reachable(id, reachable);
        }
        self.apply_signin_expired(id, outcome.signin_expired);
        self.invalidate_list_cache();
        self.rebuild_snapshot().await;
        Some(outcome)
    }

    /// Whether the mailbox list is currently empty; i.e. there is no cached/previously
    /// synced mail on screen, so a sync is a *cold* load the user is genuinely waiting on
    /// (show the download bar) rather than a hidden background refresh over visible mail.
    fn mailbox_list_is_empty(&self) -> bool {
        self.mailbox_list.get().rows.is_empty()
    }

    pub(crate) fn sync_tuning_for(&self, id: &AccountId) -> StreamTuning {
        StreamTuning::new(STREAM_FETCH_BATCH, STREAM_CHUNK_SIZE)
            .within(sync_window(self.effective_sync_depth(id.as_str())))
    }
}

fn busy_retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(
        ACCOUNT_SYNC_BUSY_RETRY_MS
            .get(attempt)
            .copied()
            .unwrap_or(10_000),
    )
}

pub(crate) fn sync_window(depth: SyncDepth) -> SyncWindow {
    let Some(date) = depth.cutoff(time::OffsetDateTime::now_utc().date()) else {
        return SyncWindow::full();
    };
    CalendarDate::new(date.year(), u8::from(date.month()), date.day())
        .map_or_else(|_| SyncWindow::full(), SyncWindow::since)
}
