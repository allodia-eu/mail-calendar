//! Settings-surface FFI methods on [`MailcalApp`]: per-account sync behaviour (strategy, poll
//! interval, push folders), sync-depth windowing, the message-list grouping, the default send
//! account, the per-direction swipe actions, the runtime log level, and the one-time offer to
//! become the OS's default mail app. Split out of `lib.rs` to
//! keep each file under the 500-line limit; the object is defined in `lib.rs`, and UniFFI
//! collects these exported methods crate-wide.

use crate::{
    DefaultMailAppOutcome, DefaultMailAppSupport, LogLevel, MailcalApp, SwipeActionKind,
    SwipeDirection, SwipeSettings, SyncSettingsSnapshot, SyncStrategyKind, ViewMode, logging,
};

#[uniffi::export]
impl MailcalApp {
    /// The per-account synchronisation-behaviour settings (pulled after a `Surface::Settings`
    /// signal): one row per account with its effective strategy (push vs. poll), poll
    /// interval, whether the server supports IMAP `IDLE`, and the folder list with
    /// push-subscription state; everything the settings screen renders.
    pub fn sync_settings(&self) -> SyncSettingsSnapshot {
        self.runtime.block_on(self.app.sync_settings()).into()
    }

    /// Sets whether an account receives mail as it arrives (`Push`, valid only when the
    /// server supports `IDLE`) or checks on a timer (`Poll`), then restarts its background
    /// sync. Persisted. Fire-and-forget beyond the persisted change; the observer fires as
    /// the new watches/poll produce mail.
    pub fn set_sync_strategy(&self, account: String, strategy: SyncStrategyKind) {
        self.runtime
            .block_on(self.app.set_sync_strategy(&account, strategy.into()));
        self.refresh_background(&account);
    }

    /// Sets an account's background-poll interval (minutes; snapped to the allowed set) and
    /// restarts its poll timer. Persisted.
    pub fn set_poll_interval(&self, account: String, minutes: u16) {
        self.runtime
            .block_on(self.app.set_poll_interval(&account, minutes));
        self.refresh_background(&account);
    }

    /// Subscribes or unsubscribes one folder for push on an account (the core caps the set
    /// at the platform-wide maximum), then restarts its watches. Persisted.
    pub fn set_push_folder(&self, account: String, folder: String, subscribed: bool) {
        self.runtime
            .block_on(self.app.set_push_folder(&account, &folder, subscribed));
        self.refresh_background(&account);
    }

    /// Sets one account's **per-account** sync depth (a month count; `0` = all mail), clears its
    /// mail cursors, and starts a re-snapshot under the new window. Widening immediately backfills
    /// older mail; narrowing tombstones rows that are no longer in scope. Fire-and-forget: the
    /// observer fires as the re-sync completes. An unknown account id is ignored.
    pub fn set_account_sync_depth(&self, account: String, months: u16) {
        let app = std::sync::Arc::clone(&self.app);
        self.runtime
            .spawn(async move { app.update_account_sync_depth(&account, months).await });
    }

    /// Sets one account's **per-account** message-size cap (a megabyte count; `0` = no limit)
    /// and acts on the mail already cached. Raising it downloads what the lower cap skipped;
    /// lowering it forgets the cached copies it may no longer keep, which runs locally and needs
    /// no server. The mail itself is never removed either way (only its offline copy) so the
    /// list, the threads and body search are unchanged. Fire-and-forget: the observer fires as
    /// the work completes. An unknown account id is ignored.
    pub fn set_account_message_size_limit(&self, account: String, megabytes: u16) {
        let app = std::sync::Arc::clone(&self.app);
        self.runtime.spawn(async move {
            app.update_account_message_size_limit(&account, megabytes)
                .await;
        });
    }

    /// The persisted message-list grouping (flat vs threaded) the settings screen renders.
    /// Changed via [`Intent::SetViewMode`](crate::Intent::SetViewMode); defaults to threaded.
    #[must_use]
    pub fn view_mode(&self) -> ViewMode {
        self.app.view_mode().into()
    }

    /// The persisted **default send account**'s id (pulled after a `Surface::Settings` signal),
    /// or `None` when the user hasn't chosen one. It decides which account a new message composes
    /// from in the unified all-inboxes view (where no selected mailbox scopes the choice) and
    /// is what the composer's From dropdown opens on there. Selecting one account's mailbox
    /// outranks it, and the dropdown's own choice (`from`) outranks both.
    ///
    /// This is the **stored** id, which may name an account the user has since removed; the core
    /// falls back to the first configured account when it no longer resolves.
    #[must_use]
    pub fn default_send_account(&self) -> Option<String> {
        self.app.default_send_account()
    }

    /// Sets and persists the default send account (`None` clears it, restoring "the first
    /// configured account"), then signals `Surface::Settings`.
    pub fn set_default_send_account(&self, account: Option<String>) {
        self.runtime
            .block_on(self.app.set_default_send_account(account));
    }

    /// The per-direction swipe actions (pulled after a `Surface::Settings` signal); what a
    /// leftward and a rightward swipe across a message row do. A host binds its row gestures to
    /// these and renders them in the settings screen. Both default to `Delete`.
    #[must_use]
    pub fn swipe_settings(&self) -> SwipeSettings {
        self.app.swipe_settings().into()
    }

    /// Sets and persists what one swipe direction does, then signals `Surface::Settings`. The two
    /// directions are configured independently.
    pub fn set_swipe_action(&self, direction: SwipeDirection, action: SwipeActionKind) {
        self.runtime
            .block_on(self.app.set_swipe_action(direction.into(), action.into()));
    }

    /// Sets the global log ceiling at runtime: a host toggling on `debug`/`trace` for a
    /// support session (or back down) without reconnecting. Affects every layer's `log`
    /// records process-wide, since the logger is a single installed sink.
    pub fn set_log_level(&self, level: LogLevel) {
        logging::set_level(level);
    }

    /// Whether to put the one-time offer to become the OS's default mail app in front of the
    /// user now.
    ///
    /// `support` is what this build can actually do about it, and `is_default` what the host
    /// could find out, with `None` where it cannot tell (a Flatpak has no host application
    /// database to ask). The core answers `false` before the first account exists, when the
    /// app is already the default, and once the offer has been answered or dismissed, so a
    /// host asks this and does not keep its own idea of when the moment is right.
    ///
    /// Setting the handler stays the host's job: this decides only whether to ask
    /// (`docs/os-integration.md`).
    #[must_use]
    pub fn should_offer_default_mail_app(
        &self,
        support: DefaultMailAppSupport,
        is_default: Option<bool>,
    ) -> bool {
        self.runtime.block_on(
            self.app
                .should_offer_default_mail_app(support.into(), is_default),
        )
    }

    /// Records what came of the offer, so it is never put again, then signals
    /// `Surface::Settings`. A prompt the user closed without answering is `Declined`.
    pub fn record_default_mail_app_offer(&self, outcome: DefaultMailAppOutcome) {
        self.runtime
            .block_on(self.app.record_default_mail_app_offer(outcome.into()));
    }

    /// What came of the offer, or `None` if it has not been put yet: `Some(true)` the user took
    /// it, `Some(false)` they turned it down. Settings reads it to say where things stand; the
    /// action it offers is the same either way.
    #[must_use]
    pub fn default_mail_app_offer(&self) -> Option<bool> {
        self.app.default_mail_app_offer()
    }
}
