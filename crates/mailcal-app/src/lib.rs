//! `mailcal-app`: the product runtime for Allodia Mail & Calendar.
//!
//! It proves the thick-core architecture in Rust: the app owns
//! one [`engine_api::Engine`] shared across every configured [`Account`] (the engine store
//! is account-scoped), drives their providers, and exposes a **unidirectional loop**; a
//! host calls [`App::dispatch`] with an [`Intent`], the app commits new state, and it
//! signals the changed [`Surface`] through an [`AppObserver`] the host then pulls a
//! snapshot for.
//!
//! Multiple accounts ride one engine: a [`Surface::MailboxList`] snapshot is either the
//! unified "all inboxes" (every account's INBOX merged, each row tagged with its account)
//! or one selected account's folders. Mail actions route to the message's **owning**
//! account; compose/new-mail uses the selected account (else the first).

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64},
    },
};

use engine_api::{AccountId, Engine, MailListRow};
use mailcal_viewmodel::{
    CalendarSnapshot, ContactsSnapshot, MailboxListSnapshot, ReadingSnapshot, ViewMode,
};
use tokio::sync::{RwLock, watch};

mod accounts;
mod avatars;
mod background_sync;
mod calendar_cache;
mod calendar_colors;
mod calendar_detail;
mod calendar_drag_ops;
mod calendar_ops;
mod calendar_prefs;
mod calendar_refresh;
mod calendar_scope;
mod calendar_status;
mod calendar_unexpandable;
mod connectivity;
mod connector;
mod contacts;
mod contacts_write;
mod dispatch;
mod display_settings;
mod folder_pane;
mod form_factor;
mod helpers;
mod html;
// The meeting-invitation card: the RSVP gate and the text/conflict rules (pure), and the
// assembly that reads the account's diary (impure). Split so the contract has tests that can fail.
mod invitations;
mod invitations_build;
#[cfg(test)]
mod invitations_conflict_tests;
#[cfg(test)]
mod invitations_ownership_tests;
// Trust, then verify: what the server reported about the reply it promised to send, and the
// offer to send it ourselves when it could not. The report itself is read in the engine, which
// is where protocol bytes belong (`ReplyDelivery`, on the write receipt).
mod invitations_fallback;
// Answering when nothing else will tell the organiser: store the meeting, store the answer,
// then send the iTIP REPLY as mail ourselves.
mod invitations_imip;
pub mod invitations_rsvp;
#[cfg(test)]
mod invitations_test_support;
#[cfg(test)]
mod invitations_tests;
// Writing iCalendar, which the engine parses but deliberately does not serialize: the
// `METHOD:REPLY` an answer travels in, and the `METHOD`-less form of an invitation that a
// calendar server will accept as a stored resource.
mod itip;
mod lifecycle;
mod live_mailbox;
mod mail_compose;
mod mail_compose_signature;
// The per-account message-size cap: what this account keeps offline, and what a change to it
// does to the mail already cached.
mod mail_ops;
mod mcp_settings;
mod message_size;
mod prefetch;
mod protocol;
mod query;
mod quote_settings;
mod reading;
mod recipients;
mod reference;
mod scope;
mod send_settings;
mod signatures;
mod snapshot;
mod snapshot_search;
mod surfaced;
mod swipe_settings;
mod sync;
mod sync_account;
mod sync_folder;
mod sync_progress;
mod sync_progress_staged;
mod sync_settings;
mod telemetry;
mod timezone;
mod tuning;
mod unfiled_copy;
mod view_settings;
mod view_snapshots;
mod window;
mod zones;

pub use accounts::Account;
use background_sync::NotifyMarksState;
pub use background_sync::{AccountNewMail, BackgroundNewMail, NewMailPreview};
pub use calendar_cache::{CalendarPage, MonthPage};
use calendar_prefs::CalendarPrefsState;
pub use connector::MailboxConnector;
pub use contacts_write::ContactTarget;
pub use display_settings::DisplaySettings;
use display_settings::DisplaySettingsState;
pub use html::{render_document, should_open_external_link};
pub use invitations_fallback::ReplyPrompt;
pub use invitations_rsvp::InvitationResponse;
pub use mail_ops::result::{MailActionError, SendActionError};
pub use mailcal_account::EventDetail;
use mcp_settings::McpSettingsState;
pub use prefetch::default_prefetch_size_limit;
pub use protocol::{
    AppObserver, CalendarWriteStatus, ComposerBlob, ContactWriteStatus, Intent,
    RecipientSuggestion, SearchScope, SendStatus, Surface,
};
pub use query::{MessageDetail, MessagePage};
use quote_settings::QuoteSettingsState;
pub use recipients::RecipientMatch;
pub use reference::{EventRef, FolderRef, MessageRef, ThreadRef};
use scope::Scope;
use send_settings::SendSettingsState;
pub use signatures::SignatureBody;
use signatures::SignatureState;
use surfaced::Surfaced;
use swipe_settings::SwipeSettingsState;
use sync_progress::SyncProgressState;
use sync_settings::SyncSettingsState;
pub use telemetry::{
    AnalyticsConsent, Batch, Context, DeviceClass, DeviceInfo, DurationBucket, Event, Feature,
    NOTICE_VERSION, PROPERTY_KEYS, Platform, Protocol, SCHEMA, Telemetry, TelemetrySink, WireEvent,
};
pub use timezone::TimeZoneInit;
use timezone::TimeZoneState;
use tuning::{
    MIN_LOAD_WINDOW, PAGE, SEARCH_FETCH_LIMIT, SEARCH_LIMIT, WINDOW_BUCKET, WINDOW_ROW_FACTOR,
};
pub use unfiled_copy::UnfiledCopy;
use view_settings::load_view_mode;
pub use zones::available_time_zones;

/// One cached mailbox-list load: the accounts it spans, the window depth it was read at, and the
/// shared, individually-`Arc`'d rows (see [`App::row_cache`]).
struct CachedRows {
    accounts: Vec<AccountId>,
    window: usize,
    rows: Arc<Vec<Arc<MailListRow>>>,
}

impl CachedRows {
    /// Whether this load answers a read of `accounts` at `window`.
    ///
    /// The accounts must match exactly: a load of one account is not the unified list, and the
    /// unified list is not one account's; while a **deeper or equal** window is a superset the
    /// view simply truncates.
    fn serves(&self, accounts: &[AccountId], window: usize) -> bool {
        self.window >= window && self.accounts == accounts
    }

    /// Whether the shown list draws from `account` at all: a commit for one it does not span
    /// changes nothing on screen.
    fn spans(&self, account: &AccountId) -> bool {
        self.accounts.contains(account)
    }

    /// An empty load for `accounts` at `window`; what a first sync splices its rows into.
    fn empty(accounts: Vec<AccountId>, window: usize) -> Self {
        Self {
            accounts,
            window,
            rows: Arc::new(Vec::new()),
        }
    }
}

/// The app runtime: owns one [`Engine`] shared across every configured
/// [`Account`], holds the surface + selection state, and notifies an [`AppObserver`]
/// when a surface changes.
pub struct App<P> {
    engine: Engine,
    /// The configured accounts, each behind an [`Arc`]. Behind an async `RwLock` because
    /// [`App::add_account`] mutates the set at runtime while the sync/snapshot/action paths
    /// read it. The `Arc` lets a reader clone one account's handle out under the guard and
    /// then **drop the guard before any network `.await`**: so a long provider round-trip
    /// never holds the (write-preferring) lock and stalls a concurrent `add_account`.
    accounts: RwLock<Vec<Arc<Account<P>>>>,
    /// What the mailbox list is showing: the unified inbox, one account, or one folder of one
    /// account. One value, so a folder key never travels without the account it belongs to
    /// (`docs/folder-pane.md`, rule 14).
    scope: Mutex<Scope>,
    mailbox_list: Surfaced<MailboxListSnapshot>,
    calendar: Surfaced<CalendarSnapshot>,
    /// The occurrences and calendars a grid pages over. Read by `calendar_page`, which is a
    /// direct query rather than a pushed snapshot: a pager renders three pages at once and
    /// one slot cannot hold three.
    calendar_cache: Mutex<calendar_cache::CalendarCache>,
    /// What the user decided about each calendar (shown/hidden and any colour override) keyed by
    /// account AND calendar id. Persisted; applied at page-read time, so a toggle in the manager
    /// redraws the grid with no re-sync.
    calendar_prefs: Mutex<CalendarPrefsState>,
    /// Which accounts have their folder tree open; persisted, and independent of selection, so
    /// navigating anywhere leaves the tree as the user left it (`docs/folder-pane.md`).
    folder_pane: Mutex<folder_pane::FolderPaneState>,
    /// The alphabetical unified-people snapshot the contacts list renders. One snapshot for
    /// **every** account, not one per account: the engine deduplicates people across accounts,
    /// so there is nothing per-account to merge here (see [`crate::contacts`]).
    contacts: Mutex<ContactsSnapshot>,
    /// The active contacts search text. Session state, never persisted: a search is a filter
    /// on one visit to the list, and re-opening Contacts should not inherit how it was last
    /// narrowed (the same rule mail search follows; see `docs/search.md`).
    contacts_query: Mutex<String>,
    /// Bumped on every contacts search. A rebuild reads it before its store read and re-checks
    /// it after, discarding a result a newer query has already superseded; intents are
    /// spawned, so two keystrokes race and the loser must not win by finishing last.
    contacts_generation: AtomicU64,
    reading: Surfaced<ReadingSnapshot>,
    /// The unanswered "shall we email the organiser ourselves?" question, if a calendar server
    /// has just failed to deliver a reply. Session state, never persisted; one surviving a
    /// restart would ask about a meeting answered last week (see [`invitations_fallback`]).
    reply_prompt: Mutex<Option<invitations_fallback::ReplyPrompt>>,
    /// The standing "your copy is not in Sent" question, until it is retried or dismissed.
    unfiled_copy: Mutex<Option<unfiled_copy::UnfiledCopy>>,
    view_mode: Mutex<ViewMode>,
    /// The preferences-file path (shared with the sub-settings states), or `None` for the
    /// in-memory demo/tests. Used to persist the message-list grouping across launches.
    prefs_path: Option<PathBuf>,
    /// The visible mailbox-list window size (rows from the top). Starts at [`PAGE`], grows by
    /// [`PAGE`] on [`Intent::ShowMore`], and resets to [`PAGE`] on any navigation that changes
    /// the list: so a folder switch always opens at the first page, and scrolling loads more.
    /// A background sync preserves it (the host keeps whatever it had scrolled into view).
    visible_limit: Mutex<usize>,
    search_query: Mutex<Option<String>>,
    /// Which folders the active search covers. Session state, never persisted: it is a filter
    /// on one search, and [`Intent::Search`]`(None)` resets it, so a search always opens on
    /// the default rather than inheriting how the last one was narrowed.
    search_scope: Mutex<SearchScope>,
    timezone: Mutex<TimeZoneState>,
    /// The persisted per-account synchronisation-behaviour choices (push vs. poll). The
    /// snapshot the host renders is assembled in [`sync_settings`](crate::sync_settings) by
    /// combining this with each account's live `IDLE` capability and folder list.
    sync_settings: Mutex<SyncSettingsState>,
    /// The persisted default reply/forward quote style, surfaced via [`Surface::Settings`]
    /// and read by the host to seed a new reply's composer (overridable per message).
    quote_settings: Mutex<QuoteSettingsState>,
    /// The persisted display preferences; first day of the week, 12/24-hour clock, and the
    /// calendar's default horizon; surfaced via [`Surface::Settings`]. The core owns them so the
    /// three clients cannot disagree about which day a week starts on.
    display_settings: Mutex<DisplaySettingsState>,
    /// The persisted default send account, surfaced via [`Surface::Settings`]. Consulted by
    /// [`App::compose_account`] only in the unified all-inboxes view, where no selected mailbox
    /// scopes the choice; an explicit `from` on a submit intent overrides it.
    send_settings: Mutex<SendSettingsState>,
    /// The persisted per-direction swipe actions (Trash / Archive / Star), surfaced via
    /// [`Surface::Settings`] and read by the host to bind its message-row swipe gestures.
    swipe_settings: Mutex<SwipeSettingsState>,
    /// The user's signature library and each account's two assignments
    /// ([`signatures`](crate::signatures)), surfaced via [`Surface::Settings`] and read by the
    /// host to seed a composer. The library lives in its own `signatures.toml`: a signature
    /// carries its images inline, and a preference write rewrites the whole file.
    signatures: Mutex<SignatureState>,
    /// The persisted local-MCP (AI assistant access) decisions: whether the server is on, which
    /// accounts it exposes, and the two send controls. Surfaced via [`Surface::Settings`]; the
    /// binding layer reads it to (re)configure the server. Off with nothing exposed by default;
    /// see [`mcp_settings`](crate::mcp_settings) for why those two defaults are the design.
    mcp_settings: Mutex<McpSettingsState>,
    /// The persisted per-account new-mail high-water-marks driving background-sync
    /// notifications ([`background_sync`](crate::background_sync)): the newest inbound-Inbox
    /// instant already reported per account, so a background pass notifies only newer mail.
    notify_marks: Mutex<NotifyMarksState>,
    /// Consented product analytics ([`telemetry`](crate::telemetry)): the persisted consent
    /// decision, the install id that consent licenses, the host's device facts, and the sink
    /// events go to. [`Telemetry::off`] in the demo/showcase/tests, nothing is ever sent then.
    /// Every emit path goes through [`App::track`], which is where the consent gate lives.
    telemetry: Telemetry,
    send_status: Surfaced<SendStatus>,
    /// The most recent calendar write's status, surfaced via [`Surface::CalendarStatus`] so a
    /// host can show a spinner while it settles and a warning when it could not be confirmed.
    calendar_write_status: Mutex<CalendarWriteStatus>,
    /// The most recent contact write's status, surfaced via [`Surface::ContactsStatus`]. Its
    /// own slot rather than the calendar's, so the contacts editor does not spin because a
    /// calendar write is settling elsewhere.
    contact_write_status: Mutex<ContactWriteStatus>,
    /// Aggregated background-sync download progress, surfaced via [`Surface::SyncProgress`].
    sync_progress: Mutex<SyncProgressState>,
    /// The host-injected port for on-demand "sync the folder you open"; `None` disables it
    /// (the demo / tests), so opening an unsynced folder just shows it empty.
    connector: Option<Box<dyn MailboxConnector<P>>>,
    /// Folders an on-demand sync has already been attempted for this session (by
    /// `(account, folder key)`), so re-selecting one does not reconnect it.
    attempted_folders: Mutex<HashSet<(String, String)>>,
    /// Accounts (by id) with a body-warming pass currently in flight, so overlapping
    /// post-sync prefetch triggers collapse into the one running drain (`prefetch`).
    prefetching: Mutex<HashSet<String>>,
    /// The largest message the body warm pulls in full, in octets; `None` warms every size.
    ///
    /// Behind a `Mutex` rather than plain, because every host holds the app as an `Arc` and
    /// so never has a `&mut` to set it through: a `&mut self` setter here is one no caller
    /// What is known about each sender's photo, keyed by canonical address.
    ///
    /// In memory only: the durable cache is the engine's, and this exists so that projecting a
    /// row is a map read rather than a store read. It repopulates cheaply after a restart;
    /// the engine's cache answers without touching a provider (`avatars`).
    avatar_photos: Mutex<HashMap<String, avatars::PhotoState>>,
    /// Whether a photo-resolution pass is in flight, so the rebuild it publishes does not
    /// start another one behind it.
    avatar_pass_running: Mutex<bool>,
    /// The shown list's cached rows: the projected rows a snapshot renders, the accounts they
    /// span and the window depth they were read at, so re-showing the same view costs nothing.
    ///
    /// `None` means "we do not currently hold the list in memory"; see
    /// [`row_cache_dropped`](Self::row_cache_dropped) for why the two ways of getting there are
    /// not the same.
    row_cache: Mutex<Option<CachedRows>>,
    /// Whether the cached list was **dropped** rather than never read.
    ///
    /// An empty [`row_cache`](Self::row_cache) alone cannot say that, and the live path
    /// (`live_mailbox`) must tell them apart. A never-loaded list is a first sync: a streamed
    /// commit *should* seed it and paint rows as they arrive, because there is nothing on screen
    /// to lose. A **dropped** one is the opposite: the list is already showing rows read from
    /// the store, so projecting it from a delta would take them off screen until the pass's
    /// authoritative rebuild put them back a beat later. Cleared by
    /// [`load_rows`](Self::load_rows) once the store read repopulates it.
    row_cache_dropped: AtomicBool,
    /// Messages (by `(account id, provider key)`) hidden from the list **optimistically** the
    /// instant the user archives/deletes one, so the row leaves the list without waiting for
    /// the move to land server-side and the re-sync to observe the expunge (IMAP deltas don't
    /// carry it, and a fresh snapshot can lag a beat). A key stays here only while the store
    /// still reports the message; [`cached_rows`](crate::App::cached_rows) drops it once the
    /// store agrees it's gone, so the set self-prunes and a failed edit reappears.
    pending_removals: Mutex<HashSet<(String, String)>>,
    /// Bumped on every [`invalidate_list_cache`](Self::invalidate_list_cache). A row load
    /// captures this before reading the store and refuses to cache its result if the value
    /// moved while the read was in flight: so a slow pre-sync load can't land after the
    /// post-sync invalidation and resurrect stale rows.
    row_cache_generation: AtomicU64,
    /// Per-account INBOX provider key, cached after folder discovery for live unified-inbox
    /// splices.
    inbox_keys: Mutex<HashMap<String, String>>,
    /// A monotonic counter bumped on every send-status change. The core's terminal
    /// auto-clear (see `mail_ops`) captures it before its delay and only resets to
    /// [`SendStatus::Idle`] if it is still unchanged: so a newer send started during the
    /// pending clear wins and is never wiped by the older send's timer.
    send_status_generation: AtomicU64,
    /// When the agent adapter's write path last drove an account-wide re-sync, or `None` if it
    /// never has. It throttles that sync to at most one per coalescing window, so a scripted
    /// "archive these fifty" costs one sync rather than fifty (`mail_ops::result`). The
    /// interactive path is unthrottled and never touches this; one user action is one refresh.
    write_refresh_at: Mutex<Option<std::time::Instant>>,
    /// Whether the device currently has network connectivity, reported by the host's OS
    /// reachability API ([`Intent::ReportNetworkReachable`]). A `watch` channel, so the
    /// background watch/poll loops can *await* the transition back online instead of
    /// busy-retrying while offline; the current value also gates the app's own syncs.
    /// Defaults to online, so a host that never reports reachability behaves as before.
    online: watch::Sender<bool>,
    /// Accounts whose most recent sync (or boot connect) couldn't reach their server **while
    /// online**, each mapped to an optional technical detail (the connect error) a host reveals
    /// behind a "details" link. Surfaced via [`Surface::Connectivity`] as per-account outage
    /// badges; distinct from a device-wide offline state (which is [`online`](Self::online)). A
    /// boot-failed account carries a rich detail; a mid-session sync failure records `None`.
    unreachable_accounts: Mutex<BTreeMap<String, Option<String>>>,
    /// Accounts whose calendar could not be connected because the OAuth grant lacks the calendar
    /// scope (a Graph `403`; connected before calendar support, or revoked consent), so the user
    /// must **re-authenticate to grant calendar access**. Distinct from
    /// [`unreachable_accounts`](Self::unreachable_accounts): mail is fully up, only the calendar
    /// is withheld, and the remedy is a re-consent, not a retry. Surfaced via
    /// [`Surface::Connectivity`] so a host shows a "reconnect for calendar" prompt.
    calendar_reauth_accounts: Mutex<BTreeSet<String>>,
    /// Accounts whose OAuth grant lacks the mail **write/send** scopes (`Mail.ReadWrite` /
    /// `Mail.Send`), so a mark-read/flag/move/delete or a send was refused with a Graph
    /// `403 ErrorAccessDenied` (an account connected before those scopes, or consent revoked
    /// server-side). Mail **reading** is unaffected: the remedy is a re-consent, not a retry;
    /// so this is raised **at the point of use** (there is no cheap boot-time write probe the way
    /// the calendar list is one), self-clears on the next successful write, and surfaces via
    /// [`Surface::Connectivity`] as a "reconnect to send and manage mail" prompt. Sibling of
    /// [`calendar_reauth_accounts`](Self::calendar_reauth_accounts): one re-consent grants the
    /// whole scope set, so re-authenticating clears both.
    mail_reauth_accounts: Mutex<BTreeSet<String>>,
    /// Accounts whose stored OAuth grant is **dead**: the refresh token expired or was revoked,
    /// so it no longer mints an access token at all (Google `invalid_grant`, a Microsoft
    /// `AADSTS700082`, a withdrawn OAuth JMAP token). Nothing syncs until the user signs in
    /// again. Distinct from the two lists above (those are missing *scopes*, with the rest of the
    /// account working) and from [`unreachable_accounts`](Self::unreachable_accounts) (the server
    /// answered; it refused the credential), so an account here is kept **out** of the outage set
    /// and surfaces via [`Surface::Connectivity`] as a "your sign-in expired; reconnect" prompt.
    signin_expired_accounts: Mutex<BTreeSet<String>>,
    observer: Arc<dyn AppObserver>,
}

impl<P> core::fmt::Debug for App<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The engine and providers may hold sensitive handles; show nothing of them.
        f.debug_struct("App").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod connectivity_prompt_tests;
#[cfg(test)]
mod connectivity_tests;
#[cfg(test)]
mod mail_ops_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_account_order;
#[cfg(test)]
mod tests_actions;
#[cfg(test)]
#[path = "tests_calendar.rs"]
mod tests_calendar;
#[cfg(test)]
mod tests_calendar_actions;
#[cfg(test)]
mod tests_calendar_drag;
#[cfg(test)]
#[path = "tests_calendar_launch.rs"]
mod tests_calendar_launch;
#[cfg(test)]
mod tests_calendar_occurrence_detail;
#[cfg(test)]
mod tests_calendar_recurrence;
#[cfg(test)]
mod tests_calendar_series_warning;
#[cfg(test)]
mod tests_calendar_unexpandable;
#[cfg(test)]
mod tests_contacts;
#[cfg(test)]
mod tests_depth;
#[cfg(test)]
mod tests_invitation_delivery;
#[cfg(test)]
mod tests_invitation_fixtures;
#[cfg(test)]
mod tests_invitation_imip;
#[cfg(test)]
mod tests_invitation_preview;
#[cfg(test)]
mod tests_invitation_rsvp;
#[cfg(test)]
mod tests_live_mailbox;
#[cfg(test)]
mod tests_mail_actions;
#[cfg(test)]
mod tests_message_size;
#[cfg(test)]
mod tests_query;
#[cfg(test)]
mod tests_reading;
#[cfg(test)]
mod tests_report;
#[cfg(test)]
mod tests_scope;
#[cfg(test)]
mod tests_search;
#[cfg(test)]
mod tests_settings;
#[cfg(test)]
mod tests_signatures;
#[cfg(test)]
mod tests_sync;
#[cfg(test)]
mod tests_telemetry;
#[cfg(test)]
mod tests_threading;
#[cfg(test)]
mod tests_warm;
