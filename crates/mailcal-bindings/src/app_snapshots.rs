//! Intent dispatch and the read-side snapshot pulls on [`MailcalApp`]: the fire-and-forget
//! `dispatch`, the per-surface snapshot getters a host reads after an observer signal, the
//! non-fatal connect-diagnostics accessors, and `reset`. Split out of `lib.rs` to keep each
//! file under the 500-line limit; the object is defined in `lib.rs`, and UniFFI collects these
//! exported methods crate-wide.

use std::sync::Arc;

use engine_api::AccountId;
use mailcal_app::Intent as AppIntent;

use crate::{
    AccountProvider, CalendarSnapshot, CalendarWriteStatus, ConnectionInfo, ConnectivitySnapshot,
    Intent, MailboxListSnapshot, MailcalApp, QuoteSettings, QuoteStyleKind, ReadingSnapshot,
    ReplyPrompt, SendStatus, SyncProgressSnapshot, TimeZoneSnapshot, UnfiledCopy, joined,
};

#[uniffi::export]
impl MailcalApp {
    /// Handles a host intent. Fire-and-forget: the work is scheduled on the internal
    /// runtime and the observer fires on completion.
    pub fn dispatch(&self, intent: Intent) {
        // A user-driven refresh, and coming back online, are the natural moments to re-dial any
        // account that has been sitting disconnected (a boot outage): so a provider coming back
        // up recovers on the next Refresh / reconnection without an app restart. A no-op when
        // nothing is disconnected. The core's own refresh (below) heals *live* dropped sockets;
        // this rebuilds accounts that never got providers at all.
        let retry_disconnected = match &intent {
            // Coming back online: the device is up (the OS just reported it) even though our own
            // `online` flag flips a beat later in the spawned handler: so always re-dial now.
            Intent::ReportNetworkReachable { reachable: true } => true,
            // A manual / boot refresh: only re-dial when we believe we're online, so repeatedly
            // pulling to refresh behind the offline banner doesn't storm a dead network each time
            // (the same offline gate `refresh_mail` itself uses).
            Intent::RefreshMail => !self.app.connectivity().offline,
            _ => false,
        };
        if retry_disconnected {
            self.retry_connections();
        }
        // A key-routed intent carries its row's owning-account id; an unparseable id (in
        // practice impossible; it is a real row's account) is dropped rather than risk
        // routing the action to the wrong account.
        let Ok(intent) = AppIntent::try_from(intent) else {
            return;
        };
        let app = Arc::clone(&self.app);
        self.runtime.spawn(async move {
            app.dispatch(intent).await;
        });
    }

    /// Refreshes calendars without recording a user action or clearing failed-write feedback.
    ///
    /// Hosts call this for timer-driven refreshes. Fire-and-forget: changed calendar surfaces are
    /// signalled through the observer after the internal runtime completes the sync.
    pub fn refresh_calendar_in_background(&self) {
        let app = Arc::clone(&self.app);
        self.runtime.spawn(async move {
            app.refresh_calendar_in_background().await;
        });
    }

    /// The technical detail behind an account's outage (its connect error), or `None` when the
    /// account is reachable: a host reveals this behind the connectivity banner's / status
    /// label's "details" link. Pulled after a `Surface::Connectivity` signal.
    #[must_use]
    pub fn connection_detail(&self, account_id: String) -> Option<String> {
        let account_id = AccountId::try_from(account_id.as_str()).ok()?;
        self.app.connection_detail(&account_id)
    }

    /// Which sign-in `account_id` was connected with, or `None` when the account is unknown.
    ///
    /// A host asks this when rendering the "your sign-in expired; reconnect" prompt for an
    /// account in `ConnectivitySnapshot::signin_expired_accounts`, since the remedy differs by
    /// family: `Microsoft` and `Google` re-run their browser sign-in with the address as a login
    /// hint, while `Password` and `Jmap` are re-entered in the account's settings. Reads the
    /// binding layer's own registry: the only layer that knows what a `dyn Provider` speaks.
    #[must_use]
    pub fn account_provider(&self, account_id: String) -> Option<AccountProvider> {
        self.registry.provider(&account_id)
    }

    /// The negotiated transport facts for `account_id`'s live providers. An empty list means the
    /// account is unknown or currently has no live providers. A missing TLS/HTTP version inside a
    /// row means "not applicable or not observed", not a connection error.
    pub fn connection_info(&self, account_id: String) -> Vec<ConnectionInfo> {
        let Ok(account_id) = AccountId::try_from(account_id.as_str()) else {
            return Vec::new();
        };
        self.runtime
            .block_on(self.app.connection_info(&account_id))
            .into_iter()
            .map(ConnectionInfo::from)
            .collect()
    }

    /// The current mailbox-list snapshot (pulled after a `surface_changed` signal).
    pub fn mailbox_list(&self) -> MailboxListSnapshot {
        self.app.mailbox_list().into()
    }

    /// The current calendar agenda snapshot (pulled after a `Surface::Calendar` signal).
    pub fn calendar_list(&self) -> CalendarSnapshot {
        self.app.calendar_list().into()
    }

    /// The current reading-view snapshot (pulled after a `Surface::Reading` signal): the
    /// open message's key and its fetched body. `html` is sanitised in the core (scripts
    /// and remote images stripped); a host still renders it in a WebView with scripting
    /// off and remote loads blocked, falling back to `plain` when `html` is `None`.
    pub fn reading_view(&self) -> ReadingSnapshot {
        self.app.reading_view().into()
    }

    /// The current background mail-download progress (pulled after a
    /// `Surface::SyncProgress` signal): a host shows a "downloading Y of X" bar while
    /// `active`, hiding it once the pass completes.
    pub fn sync_progress(&self) -> SyncProgressSnapshot {
        self.app.sync_progress().into()
    }

    /// The current connectivity state (pulled after a `Surface::Connectivity` signal): the
    /// device-offline flag (a host shows a global banner) and, while online, the ids of
    /// accounts whose last sync couldn't reach their server (a host badges each in the
    /// switcher). Dispatch `Intent::ReportNetworkReachable` to feed the OS reachability signal.
    pub fn connectivity(&self) -> ConnectivitySnapshot {
        self.app.connectivity().into()
    }

    /// The non-fatal account-connect diagnostics (account-prefixed, newline-joined), or
    /// `None` when there are none: any stored account skipped at launch because its mail
    /// connect failed (a stale password, a server blip). The host reads this after
    /// construction to surface that some accounts couldn't connect and were skipped. Distinct
    /// from [`MailcalApp::calendar_connect_error`], which reports CalDAV-only failures.
    pub fn account_connect_error(&self) -> Option<String> {
        joined(&self.account_connect_errors)
    }

    /// The non-fatal calendar (CalDAV) connect diagnostics (account-prefixed, newline-joined),
    /// or `None` when there are none: an account whose mail is up but whose configured calendar
    /// provider couldn't connect (so its calendar is empty, not missing by choice). The host
    /// reads this after construction and after [`MailcalApp::add_account`] to explain an empty
    /// calendar. Distinct from [`MailcalApp::account_connect_error`], which reports skipped
    /// accounts whose mail connect failed.
    pub fn calendar_connect_error(&self) -> Option<String> {
        joined(&self.calendar_connect_errors)
    }

    /// The current display-timezone setting (pulled after a `Surface::Settings` signal):
    /// the active zone and any pending device-zone change the host prompts on.
    pub fn timezone_settings(&self) -> TimeZoneSnapshot {
        self.app.timezone_settings().into()
    }

    /// The reply/forward quoting settings (pulled after a `Surface::Settings` signal): the
    /// default style the host seeds a new reply's composer with, and whether the composer
    /// offers a per-message override of it. When `per_message` is false (the default) the
    /// client must not show a style picker in the composer.
    pub fn quote_settings(&self) -> QuoteSettings {
        self.app.quote_settings().into()
    }

    /// Sets and persists the default reply/forward quote style, then signals
    /// `Surface::Settings`. Fire-and-forget beyond the persisted change.
    pub fn set_quote_style(&self, style: QuoteStyleKind) {
        self.runtime
            .block_on(self.app.set_default_quote_style(style.into()));
    }

    /// Sets and persists whether the composer offers a per-message quote-style override, then
    /// signals `Surface::Settings`. Fire-and-forget beyond the persisted change.
    pub fn set_quote_style_per_message(&self, per_message: bool) {
        self.runtime
            .block_on(self.app.set_quote_style_per_message(per_message));
    }

    /// The current outgoing-send status (pulled after a `Surface::Sending` signal): the
    /// host shows `Sending` while a send is in flight and the terminal `Sent`/`Failed`
    /// briefly once it completes.
    pub fn send_status(&self) -> SendStatus {
        self.app.send_status().into()
    }

    /// The current calendar-write status (pulled after a `Surface::CalendarStatus` signal): a
    /// host shows a small in-calendar spinner while `Saving` and, briefly, the terminal state
    /// (a check on `Saved`, a warning on `Failed`). `Failed` means "could not confirm", not
    /// "rejected"; see [`CalendarWriteStatus`].
    pub fn calendar_write_status(&self) -> CalendarWriteStatus {
        self.app.calendar_write_status().into()
    }

    /// The unanswered question raised when a calendar server reported it could not deliver an
    /// invitation reply (pulled after a `Surface::InvitationReply` signal), or `None` when
    /// there is nothing to ask.
    ///
    /// A host shows this as a modal: the answer is already saved, but the organiser has not
    /// been told, and we can email them instead. `None` also means *close the modal*: the core
    /// clears the prompt the moment it is answered, so a stale one cannot be answered twice.
    pub fn reply_prompt(&self) -> Option<ReplyPrompt> {
        self.app.reply_prompt().map(Into::into)
    }

    /// The message that was sent without its copy reaching the account's Sent folder (pulled
    /// after a `Surface::UnfiledCopy` signal), or `None` when there is nothing to ask.
    ///
    /// A host shows this as a modal offering to file the copy. `None` also means *close the
    /// modal*: the core clears it the moment the copy lands or the user dismisses it.
    pub fn unfiled_copy(&self) -> Option<UnfiledCopy> {
        self.app.unfiled_copy().map(Into::into)
    }

    /// Resets the account: clears the local cache and re-syncs from scratch, re-fetching
    /// and re-normalising everything. **Destructive**; discards the local cache (the
    /// durable outbox is kept). Fire-and-forget: the observer fires when the re-sync
    /// completes and the host pulls the refreshed snapshot.
    pub fn reset(&self) {
        let app = Arc::clone(&self.app);
        self.runtime.spawn(async move {
            app.reset().await;
        });
    }
}
