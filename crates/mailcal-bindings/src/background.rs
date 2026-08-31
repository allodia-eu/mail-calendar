//! The per-account background sync runtime: standing IMAP `IDLE` watches (push) and
//! periodic poll timers, driven off the bindings' tokio runtime.
//!
//! This is the *runtime* half of the synchronisation-behaviour feature; the *configuration*
//! (which account pushes vs. polls, which folders) lives in the product core's
//! `sync_settings` state machine. The manager reads the core's [`SyncSettingsSnapshot`] to
//! decide what to run, then spawns one task per watched folder (push) or one timer per
//! account (poll). It lives here, not in the core, because building an [`ImapWatcher`] (and
//! polling) needs the account credentials (the [`SharedRegistry`] registry) and the runtime
//! the bindings own: the core stays generic over `Provider` and credential-free.
//!
//! Cancellation is by aborting the per-account [`JoinHandle`]s: a settings change re-applies
//! the account (abort + respawn), and process teardown drops the runtime (which aborts every
//! task). The host loop follows the engine's prescription (`engine_provider::Watch`): sync
//! once before trusting a watch, sync on each `Changed`, reconnect with backoff on error.

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use engine_api::{AccountId, Provider};
use engine_provider::WatchEvent;
use mailcal_app::App;
use mailcal_viewmodel::{AccountSyncRow, SyncStrategyKind};
use tokio::{runtime::Handle, sync::watch, task::JoinHandle};

use crate::SharedRegistry;

/// The app type every account shares (providers boxed behind the trait).
type SharedApp = Arc<App<Box<dyn Provider>>>;

/// The first reconnect delay after a dropped/failed watch; doubled each further failure up
/// to [`RECONNECT_BACKOFF_MAX`]. Small so a healthy connection that blips recovers quickly.
const RECONNECT_BACKOFF_BASE: Duration = Duration::from_secs(2);

/// The ceiling on the exponential reconnect backoff, so a server that stays down settles into
/// a calm once-a-minute retry rather than the old fixed 15s busy-loop.
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(60);

/// The largest power the base is doubled by (2^5 = 32× ⇒ 64s, clamped to the max). Keeps the
/// shift well clear of overflow regardless of how long an outage lasts.
const BACKOFF_MAX_EXP: u32 = 5;

/// How long a watch must stay connected before we treat it as **healthy** and clear the backoff.
/// A drop after this counts as a transient blip (reconnect promptly); a drop sooner is a flapping
/// server (a proxy that accepts the login then kills IDLE at once); keep escalating so we don't
/// re-dial it every couple of seconds.
const HEALTHY_WATCH_UPTIME: Duration = Duration::from_secs(30);

/// Capped exponential reconnect backoff with a small per-watch offset. The exponential curve
/// stops a flapping server or a long outage from busy-looping; the deterministic per-watch
/// offset staggers many folders so they don't all reconnect in the same instant (a thundering
/// herd on the server and the local resolver). Reset on every successful connect.
struct Backoff {
    attempt: u32,
    offset: Duration,
}

impl Backoff {
    /// Seeds the per-watch offset from `seed` (the folder key), so each folder staggers
    /// differently but deterministically (stable logs, unit-testable).
    fn new(seed: &str) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        seed.hash(&mut hasher);
        Self {
            attempt: 0,
            offset: Duration::from_millis(hasher.finish() % 1000),
        }
    }

    /// Clears the backoff after a successful connect, so the next drop retries promptly.
    fn reset(&mut self) {
        self.attempt = 0;
    }

    /// The next delay; `base · 2^attempt` capped at [`RECONNECT_BACKOFF_MAX`], plus the
    /// per-watch offset, and advances the attempt counter (saturating at the cap).
    fn next_delay(&mut self) -> Duration {
        let exp = self.attempt.min(BACKOFF_MAX_EXP);
        let capped = RECONNECT_BACKOFF_BASE
            .saturating_mul(1 << exp)
            .min(RECONNECT_BACKOFF_MAX);
        self.attempt = self.attempt.saturating_add(1);
        capped + self.offset
    }
}

/// Waits until the device is online, returning immediately if it already is. Used before each
/// connect attempt so the watch loop doesn't hammer a dead network while the device is
/// offline (the every-15s DNS-failure storm) and resumes the instant connectivity returns. A
/// closed channel (the app shutting down) falls through so the caller exits naturally.
async fn await_online(online: &mut watch::Receiver<bool>) {
    let _ = online.wait_for(|&online| online).await;
}

/// Owns the live background tasks (push watches + poll timers), keyed by account id, and
/// (re)spawns them from the current settings snapshot.
pub(crate) struct BackgroundManager {
    app: SharedApp,
    registry: SharedRegistry,
    handle: Handle,
    /// The running tasks per account; aborted and replaced on each [`apply`](Self::apply).
    tasks: Mutex<HashMap<String, Vec<JoinHandle<()>>>>,
}

impl BackgroundManager {
    /// Builds a manager over the shared app, the account-config registry (for watcher
    /// credentials), and the runtime handle it spawns tasks on.
    pub(crate) fn new(app: SharedApp, registry: SharedRegistry, handle: Handle) -> Self {
        Self {
            app,
            registry,
            handle,
            tasks: Mutex::new(HashMap::new()),
        }
    }

    /// (Re)applies one account's background work: aborts whatever is running for it, then
    /// spawns watches (push) or a poll timer per the `row`. `None` (the account has no
    /// settings row; e.g. it was removed) just stops it.
    pub(crate) fn apply(&self, account_id: &str, row: Option<&AccountSyncRow>) {
        self.stop(account_id);
        let Some(row) = row else {
            return;
        };
        let mut handles = Vec::new();
        match row.strategy {
            SyncStrategyKind::Push => {
                // One standing IDLE connection per subscribed folder (capped in the core).
                for folder in row.folders.iter().filter(|folder| folder.subscribed) {
                    handles.push(self.handle.spawn(watch_loop(
                        Arc::clone(&self.app),
                        Arc::clone(&self.registry),
                        account_id.to_owned(),
                        folder.key.clone(),
                    )));
                }
            }
            SyncStrategyKind::Poll => {
                handles.push(self.handle.spawn(poll_loop(
                    Arc::clone(&self.app),
                    account_id.to_owned(),
                    row.poll_interval_mins,
                )));
            }
        }
        if !handles.is_empty() {
            self.tasks
                .lock()
                .expect("background-tasks mutex poisoned")
                .insert(account_id.to_owned(), handles);
        }
    }

    /// Aborts and forgets every running task for one account.
    fn stop(&self, account_id: &str) {
        if let Some(handles) = self
            .tasks
            .lock()
            .expect("background-tasks mutex poisoned")
            .remove(account_id)
        {
            for handle in handles {
                handle.abort();
            }
        }
    }
}

/// A standing IMAP `IDLE` watch on one folder: connect, sync once, then sync on every
/// `Changed`; on a dropped connection reconnect after a backoff. Runs until aborted.
async fn watch_loop(
    app: SharedApp,
    registry: SharedRegistry,
    account_id: String,
    folder_key: String,
) {
    let mut online = app.online_signal();
    let mut backoff = Backoff::new(&folder_key);
    let log_scope = watch_log_scope(&account_id, &folder_key);
    loop {
        // Don't even attempt a connect while the device is offline; park until it returns,
        // so an overnight outage doesn't retry every few seconds against a dead resolver.
        await_online(&mut online).await;
        let Some(config) = registry.imap_config(&account_id) else {
            // The account's config is gone (removed), or it's a Microsoft account (Graph
            // has no IMAP IDLE; it polls), nothing to watch here.
            return;
        };
        match mailcal_account::connect_imap_watcher(&config, &folder_key).await {
            Ok(mut watch) => {
                let connected_at = Instant::now();
                // Sync once before trusting the watch, to catch anything that changed while
                // we were not idling (the engine's prescribed host loop).
                sync_watched(&app, &account_id, &folder_key).await;
                loop {
                    match watch.next().await {
                        Ok(WatchEvent::Changed) => {
                            sync_watched(&app, &account_id, &folder_key).await;
                        }
                        // KeepAlive (and any future non-exhaustive variant): the watch is
                        // healthy and re-issued IDLE, nothing to do until the next change.
                        Ok(_) => {}
                        Err(err) => {
                            log::warn!("watch[{log_scope}] dropped ({err}); reconnecting",);
                            break;
                        }
                    }
                }
                // Clear the backoff only if the watch stayed up long enough to be healthy; a
                // connection that dropped almost immediately is a flapping server, so keep the
                // escalated delay rather than resetting to the 2s base and busy-looping it.
                if connected_at.elapsed() >= HEALTHY_WATCH_UPTIME {
                    backoff.reset();
                }
            }
            Err(err) => {
                log::warn!("watch[{log_scope}] connect failed ({err}); retrying");
            }
        }
        // Back off before the next attempt (escalating while the failure persists); if the
        // device goes offline during the wait, the top-of-loop gate parks the retry entirely.
        tokio::time::sleep(backoff.next_delay()).await;
    }
}

fn watch_log_scope(account_id: &str, folder_key: &str) -> String {
    format!(
        "{}/{folder_key}",
        mailcal_account::account_log_handle(account_id)
    )
}

/// Re-syncs the watched folder through the core (parsing the account id once per call).
async fn sync_watched(app: &SharedApp, account_id: &str, folder_key: &str) {
    if let Ok(id) = AccountId::try_from(account_id) {
        app.sync_watched_folder(&id, folder_key).await;
    }
}

/// A periodic per-account poll: refresh the account every `minutes`, starting hidden and showing
/// progress only if mail is actually downloaded. Runs until aborted. The immediate first tick is
/// consumed (boot/add already synced the account).
async fn poll_loop(app: SharedApp, account_id: String, minutes: u16) {
    let period = Duration::from_secs(u64::from(minutes) * 60);
    let mut interval = tokio::time::interval(period);
    interval.tick().await;
    loop {
        interval.tick().await;
        // `refresh_account` itself no-ops while offline, so the tick stays cheap, when online
        // returns, the app's own reachability handler refreshes every account, so the poll
        // needs no offline gate of its own.
        if let Ok(id) = AccountId::try_from(account_id.as_str()) {
            app.refresh_account(&id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_watch_log_scope_does_not_expose_the_account_address() {
        let scope = watch_log_scope("alice@example.test@imap.example.test", "folder-opaque-key");

        assert!(scope.starts_with("acct:"));
        assert!(scope.ends_with("/folder-opaque-key"));
        assert!(!scope.contains("alice"));
        assert!(!scope.contains('@'));
    }

    #[test]
    fn backoff_escalates_then_settles_at_the_cap() {
        let mut backoff = Backoff::new("INBOX");
        let offset = backoff.offset;
        // base·2^0, ·2^1, ·2^2 …
        assert_eq!(backoff.next_delay(), RECONNECT_BACKOFF_BASE + offset);
        assert_eq!(backoff.next_delay(), RECONNECT_BACKOFF_BASE * 2 + offset);
        assert_eq!(backoff.next_delay(), RECONNECT_BACKOFF_BASE * 4 + offset);
        // …then every later delay is pinned to the cap (never the old busy-loop, never
        // unbounded growth).
        for _ in 0..12 {
            assert!(backoff.next_delay() <= RECONNECT_BACKOFF_MAX + offset);
        }
        assert_eq!(backoff.next_delay(), RECONNECT_BACKOFF_MAX + offset);
    }

    #[test]
    fn reset_returns_to_the_base_delay() {
        let mut backoff = Backoff::new("Sent");
        let offset = backoff.offset;
        for _ in 0..8 {
            backoff.next_delay();
        }
        backoff.reset();
        assert_eq!(backoff.next_delay(), RECONNECT_BACKOFF_BASE + offset);
    }

    #[test]
    fn the_per_watch_offset_is_bounded_and_stable() {
        // Under a second, and identical across restarts for the same folder (so staggering is
        // deterministic and logs stay reproducible).
        assert!(Backoff::new("INBOX").offset < Duration::from_secs(1));
        assert_eq!(Backoff::new("INBOX").offset, Backoff::new("INBOX").offset);
    }
}
