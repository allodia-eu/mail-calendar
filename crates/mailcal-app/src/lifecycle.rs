//! Building the app, and putting it back to a cold start.
//!
//! The constructor is the one place every piece of persisted state is wired to the preferences
//! path, so it reads as the inventory of what the app remembers; `reset` is its opposite. Both
//! live here rather than in `lib.rs` because that file is the crate's map: the module tree, the
//! `App` struct and its fields, and behaviour in it is behaviour with no topic.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64},
    },
};

use engine_api::{Engine, Provider};
use mailcal_viewmodel::ContactsSnapshot;
use tokio::sync::{RwLock, watch};

use crate::{
    Account, App, AppObserver, CalendarWriteStatus, MailboxConnector, PAGE, SearchScope, Surface,
    Telemetry, TimeZoneInit, background_sync::NotifyMarksState, calendar_cache,
    calendar_prefs::CalendarPrefsState, display_settings::DisplaySettingsState, folder_pane,
    load_view_mode, mcp_settings::McpSettingsState, quote_settings::QuoteSettingsState,
    scope::Scope, send_settings::SendSettingsState, signatures::SignatureState, surfaced::Surfaced,
    swipe_settings::SwipeSettingsState, sync, sync_progress::SyncProgressState,
    sync_settings::SyncSettingsState, timezone::TimeZoneState,
};

impl<P: Provider> App<P> {
    /// Builds the app over an opened `engine` and the initial set of `accounts` it drives
    /// (often one: the host adds more later via [`App::add_account`]). Starts in the unified
    /// all-inboxes view, grouped per the persisted message-grouping preference (default
    /// [`ViewMode::Threaded`](mailcal_viewmodel::ViewMode::Threaded)).
    ///
    /// `timezone` carries the host-reported OS zone and the persistence path
    /// ([`TimeZoneInit`]): on first boot the device zone is adopted and saved; a stored
    /// zone that differs from the device raises a pending change the host prompts on.
    ///
    /// `telemetry` carries the consented-analytics wiring: the host's device facts and the
    /// sink events go to. Pass [`Telemetry::off`] to disable analytics outright (the demo, the
    /// showcase, and every test), in which case nothing is ever built or sent.
    #[must_use]
    pub fn new(
        engine: Engine,
        accounts: Vec<Account<P>>,
        timezone: TimeZoneInit,
        connector: Option<Box<dyn MailboxConnector<P>>>,
        observer: Arc<dyn AppObserver>,
        telemetry: Telemetry,
    ) -> Self {
        // The display-zone, sync, quote-style, send-account, swipe-action, and message-grouping
        // settings share one preferences file, so clone the path before `TimeZoneState` consumes
        // it.
        let prefs_path = timezone.prefs_path.clone();
        // The signature library is a sibling file of the preferences, derived rather than passed
        // in: it is the same app data directory, and every caller (host, demo, showcase, tests)
        // would otherwise have to learn a second path to say the same thing. No preferences path
        // means no persistence at all (the in-memory demo and the tests), which is exactly what a
        // signature library should do there too.
        let signatures_path = prefs_path
            .as_ref()
            .and_then(|path| path.parent())
            .map(mailcal_account::signatures_path);
        // The message-list grouping is a persisted app preference (default Threaded); seed the
        // runtime mode from it so the choice survives a restart.
        let view_mode = load_view_mode(prefs_path.as_ref());
        Self {
            engine,
            accounts: RwLock::new(accounts.into_iter().map(Arc::new).collect()),
            scope: Mutex::new(Scope::default()),
            mailbox_list: Surfaced::new(Surface::MailboxList, Arc::clone(&observer)),
            calendar: Surfaced::new(Surface::Calendar, Arc::clone(&observer)),
            calendar_cache: Mutex::new(calendar_cache::CalendarCache::default()),
            calendar_prefs: Mutex::new(CalendarPrefsState::new(prefs_path.clone())),
            folder_pane: Mutex::new(folder_pane::FolderPaneState::new(prefs_path.clone())),
            contacts: Mutex::new(ContactsSnapshot::default()),
            contacts_query: Mutex::new(String::new()),
            contacts_generation: AtomicU64::new(0),
            reading: Surfaced::new(Surface::Reading, Arc::clone(&observer)),
            reply_prompt: Mutex::new(None),
            unfiled_copy: Mutex::new(None),
            view_mode: Mutex::new(view_mode),
            prefs_path: prefs_path.clone(),
            visible_limit: Mutex::new(PAGE),
            search_query: Mutex::new(None),
            search_scope: Mutex::new(SearchScope::default()),
            timezone: Mutex::new(TimeZoneState::new(
                timezone.device_zone,
                timezone.prefs_path,
            )),
            quote_settings: Mutex::new(QuoteSettingsState::new(prefs_path.clone())),
            display_settings: Mutex::new(DisplaySettingsState::new(prefs_path.clone())),
            send_settings: Mutex::new(SendSettingsState::new(prefs_path.clone())),
            swipe_settings: Mutex::new(SwipeSettingsState::new(prefs_path.clone())),
            signatures: Mutex::new(SignatureState::new(
                signatures_path.clone(),
                prefs_path.clone(),
            )),
            mcp_settings: Mutex::new(McpSettingsState::new(prefs_path.clone())),
            sync_settings: Mutex::new(SyncSettingsState::new(prefs_path.clone())),
            notify_marks: Mutex::new(NotifyMarksState::new(prefs_path)),
            telemetry,
            send_status: Surfaced::new(Surface::Sending, Arc::clone(&observer)),
            calendar_write_status: Mutex::new(CalendarWriteStatus::default()),
            sync_progress: Mutex::new(SyncProgressState::default()),
            connector,
            attempted_folders: Mutex::new(HashSet::new()),
            prefetching: Mutex::new(HashSet::new()),
            avatar_photos: Mutex::new(HashMap::new()),
            avatar_pass_running: Mutex::new(false),
            row_cache: Mutex::new(None),
            row_cache_dropped: AtomicBool::new(false),
            pending_removals: Mutex::new(HashSet::new()),
            row_cache_generation: AtomicU64::new(0),
            inbox_keys: Mutex::new(HashMap::new()),
            send_status_generation: AtomicU64::new(0),
            write_refresh_at: Mutex::new(None),
            online: watch::channel(true).0,
            unreachable_accounts: Mutex::new(BTreeMap::new()),
            calendar_reauth_accounts: Mutex::new(BTreeSet::new()),
            mail_reauth_accounts: Mutex::new(BTreeSet::new()),
            signin_expired_accounts: Mutex::new(BTreeSet::new()),
            observer,
        }
    }

    /// Resets every account: clears the local cache so the next sync re-fetches and
    /// re-normalises everything, then re-syncs to repopulate. Destructive; discards the
    /// cached state (the durable outbox is kept). A host's "reset / full refetch".
    ///
    /// The re-sync's reconciling snapshot **hard-deletes** every message now outside the
    /// sync-depth window, but SQLite keeps the file at its high-water mark and reuses the
    /// freed pages: so the database would stay at its pre-reset peak. Compact it **after**
    /// the re-sync has settled (the deletions are committed by then), shrinking the file back
    /// to the working set. VACUUM is heavy (it rewrites the database), so it runs once here,
    /// off the sync hot path; never per sync.
    pub async fn reset(&self) {
        let _ = self.engine.reset().await;
        // A reset is an explicit, user-awaited full re-download, so show the progress bar
        // even though the pre-reset snapshot is still painted (`awaited: true`).
        self.refresh_mail(sync::RefreshProgress::Awaited).await;
        let _ = self.engine.vacuum().await;
    }
}
