//! The per-account background sync runtime: standing IMAP `IDLE` watches (push) and
//! periodic poll timers, driven off the bindings' tokio runtime.
//!
//! This is the *runtime* half of the synchronisation-behaviour feature; the *configuration*
//! (which account pushes vs. polls, which folders) lives in the product core's
//! `sync_settings` state machine. The manager reads the core's [`SyncSettingsSnapshot`] to
//! decide what to run, then spawns one task per watched folder plus a full pass every
//! [`PUSH_FULL_PASS_MINUTES`] (push), or one timer per account (poll). It lives here, not in the
//! core, because building an [`ImapWatcher`] (and polling) needs the account credentials (the
//! [`SharedRegistry`] registry) and the runtime the bindings own: the core stays generic over
//! `Provider` and credential-free.
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

/// How often an account that receives push also runs a full pass, in minutes.
///
/// A watch covers only the folder it names, and an account watches four at most, so without this
/// every other folder (and the unread count beside it) would be as current as the pass at launch
/// or at the last reconnect: for a desktop left running, days. The shortest interval an account
/// that polls can choose, which is also Android's floor for periodic work.
const PUSH_FULL_PASS_MINUTES: u16 = mailcal_account::POLL_INTERVALS[0];

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
    /// spawns watches and a full-pass timer (push) or a poll timer per the `row`. `None` (the
    /// account has no settings row; e.g. it was removed) just stops it.
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
                // Every folder the watches do not name, on the same loop a polling account runs.
                handles.push(self.handle.spawn(poll_loop(
                    Arc::clone(&self.app),
                    account_id.to_owned(),
                    PUSH_FULL_PASS_MINUTES,
                )));
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

    /// [`apply`](Self::apply) with the account's row from the settings in effect now.
    pub(crate) async fn apply_current(&self, account_id: &str) {
        let snapshot = self.app.sync_settings().await;
        let row = snapshot
            .accounts
            .iter()
            .find(|row| row.account_id == account_id);
        self.apply(account_id, row);
    }

    /// How many tasks run for one account: a watch per folder and the full-pass timer, or one
    /// poll timer.
    #[cfg(test)]
    pub(crate) fn task_count(&self, account_id: &str) -> usize {
        self.tasks
            .lock()
            .expect("background-tasks mutex poisoned")
            .get(account_id)
            .map_or(0, Vec::len)
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

/// An added account's first sync, with its background work started between the mail and the
/// warm-up. Before the mail sync a new account has no stored folders, so there is none to watch;
/// after the warm-up the watches would wait for every body in the window to be cached, which on a
/// large mailbox is many minutes of new mail arriving unannounced.
pub(crate) async fn sync_added_account(
    app: &SharedApp,
    background: &BackgroundManager,
    id: &AccountId,
) {
    app.sync_added_mail(id).await;
    background.apply_current(id.as_str()).await;
    app.warm_added_account(id).await;
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
    // The folder's refreshes run beside the watch, never in it: one can wait minutes for a folder
    // another sync holds, and the watch has to be back in IDLE meanwhile to hear the next change.
    let refresher = Refresher::spawn({
        let (app, account_id, folder_key) =
            (Arc::clone(&app), account_id.clone(), folder_key.clone());
        move || {
            let (app, account_id, folder_key) =
                (Arc::clone(&app), account_id.clone(), folder_key.clone());
            async move { sync_watched(&app, &account_id, &folder_key).await }
        }
    });
    loop {
        // Don't even attempt a connect while the device is offline; park until it returns,
        // so an overnight outage doesn't retry every few seconds against a dead resolver.
        await_online(&mut online).await;
        let Some(((config, connections, tokens), shared)) = registry.imap_store(&account_id) else {
            // The account's config is gone (removed), or it's a Microsoft account (Graph
            // has no IMAP IDLE; it polls), nothing to watch here.
            return;
        };
        match mailcal_account::connect_imap_watcher(
            &connections,
            &config,
            tokens.as_ref(),
            &folder_key,
            shared.as_ref(),
        )
        .await
        {
            Ok(mut watch) => {
                // Sync once before trusting the watch, to catch anything that changed while
                // we were not idling (the engine's prescribed host loop).
                refresher.request();
                let idling_since = Instant::now();
                loop {
                    match watch.next().await {
                        Ok(WatchEvent::Changed) => refresher.request(),
                        // KeepAlive (and any future non-exhaustive variant): the watch is
                        // healthy and re-issued IDLE, nothing to do until the next change.
                        Ok(_) => {}
                        Err(err) => {
                            let uptime = idling_since.elapsed();
                            if ended_routinely(uptime, err.class()) {
                                log::info!(
                                    "watch[{log_scope}] closed after {}s ({err}); reconnecting",
                                    uptime.as_secs(),
                                );
                            } else {
                                log::warn!("watch[{log_scope}] dropped ({err}); reconnecting");
                            }
                            break;
                        }
                    }
                }
                // Clear the backoff only if the watch stayed up long enough to be healthy; a
                // connection that dropped almost immediately is a flapping server, so keep the
                // escalated delay rather than resetting to the 2s base and busy-looping it.
                if idling_since.elapsed() >= HEALTHY_WATCH_UPTIME {
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

/// Whether a watch that idled for `uptime` and then failed with `class` ended routinely: a session
/// past [`HEALTHY_WATCH_UPTIME`] ending in a retryable failure. That is the `* BYE` some servers
/// send after a fixed time, and equally a connection the network dropped, which the class does not
/// tell apart, so the line names neither; a reconnect that then fails warns on its own. A watch
/// that dropped almost at once, or failed for a reason a reconnect will not cure, is a warning.
fn ended_routinely(uptime: Duration, class: engine_api::FailureClass) -> bool {
    uptime >= HEALTHY_WATCH_UPTIME && class == engine_api::FailureClass::Retryable
}

fn watch_log_scope(account_id: &str, folder_key: &str) -> String {
    format!(
        "{}/{folder_key}",
        mailcal_account::account_log_handle(account_id)
    )
}

/// One watched folder's refreshes, run by a task of their own so the watch never waits on one.
///
/// A request that arrives while a refresh runs is kept, and requests that arrive together are one:
/// [`Notify`](tokio::sync::Notify) holds a single wake-up for a waiter that is not waiting yet. So
/// a burst of changes during a long refresh costs exactly one more refresh, which reads all of
/// them. Dropping the refresher, with the watch loop that owns it, stops the task.
struct Refresher {
    wake: Arc<tokio::sync::Notify>,
    task: JoinHandle<()>,
}

impl Refresher {
    fn spawn<F, Fut>(refresh: F) -> Self
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let wake = Arc::new(tokio::sync::Notify::new());
        let task = tokio::spawn({
            let wake = Arc::clone(&wake);
            async move {
                loop {
                    wake.notified().await;
                    refresh().await;
                }
            }
        });
        Self { wake, task }
    }

    /// Asks for a refresh: now, or once the one running finishes.
    fn request(&self) {
        self.wake.notify_one();
    }
}

impl Drop for Refresher {
    fn drop(&mut self) {
        self.task.abort();
    }
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
            // Where the provider named a window, sit out the rest of it before the next tick.
            // The engine reports a long refusal instead of sleeping a task through it, so
            // honouring the number is this loop's job; ignoring it would walk the next tick
            // into the same refusal and spend a round trip learning nothing. Shorter than the
            // poll period is already covered by the interval, so only the excess is waited.
            if let Some(asked) = app.refresh_account(&id).await
                && let Some(excess) = asked.checked_sub(period)
            {
                log::info!(
                    "{}: rate limited; holding the poll a further {}s the server asked for",
                    poll_log_scope(&account_id),
                    excess.as_secs(),
                );
                tokio::time::sleep(excess).await;
                // The interval kept ticking while we slept; drop the backlog so returning
                // does not immediately fire the tick this wait exists to postpone.
                interval.reset();
            }
        }
    }
}

/// A poll loop's log scope: the account, hashed, never its address (`docs/logging.md`).
fn poll_log_scope(account_id: &str) -> String {
    format!("poll:{}", mailcal_account::account_log_handle(account_id))
}

#[cfg(test)]
#[path = "background_tests.rs"]
mod tests;
