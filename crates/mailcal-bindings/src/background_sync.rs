//! FFI for one-shot background sync (`docs/background-sync.md`): the
//! [`MailcalApp::run_background_sync`] method a host calls from its OS background scheduler
//! (Android WorkManager / iOS `BGAppRefreshTask`), the [`MailcalApp::new_background_worker`]
//! headless constructor a cold worker builds the core with, and the `uniffi::Record` result types
//! mirroring `mailcal_app`'s `BackgroundNewMail`. Co-located here so `records.rs`/`convert.rs` stay
//! untouched.

use std::{sync::Arc, time::Duration};

use mailcal_app::{
    AccountNewMail as AppAccountNewMail, BackgroundNewMail as AppBackgroundNewMail,
    NewMailPreview as AppNewMailPreview,
};

use crate::{DeviceInfo, LogLevel, Logger, MailcalApp, MailcalError, Observer, boot};

/// The floor for a background-sync budget (seconds): below this a pass can't meaningfully
/// connect and sync, so a smaller host value is clamped up.
const MIN_BUDGET_SECS: u64 = 5;

/// The ceiling for a background-sync budget (seconds): iOS grants a `BGAppRefreshTask` only
/// ~30 s and even a generous Android worker has minutes, so clamp well under any OS hard kill.
const MAX_BUDGET_SECS: u64 = 170;

/// One newly-arrived inbound message, for a host to raise a local notification from.
#[derive(uniffi::Record)]
pub struct NewMailPreview {
    /// The first sender address (empty if the header supplied none).
    pub sender: String,
    /// The sender's display name, when the header supplied one: a friendlier notification
    /// title than the bare address.
    pub sender_name: Option<String>,
    /// The subject (empty if none).
    pub subject: String,
    /// The received instant, RFC3339 (`…Z`); empty if the message carried no date.
    pub received: String,
    /// The message's stable provider key: for OS-notification dedupe and a tap deep-link.
    pub message_key: String,
}

/// The new inbound Inbox mail one account received during a background pass.
#[derive(uniffi::Record)]
pub struct AccountNewMail {
    /// The id of the account the mail arrived on.
    pub account_id: String,
    /// The account's address (its login identity), for a per-account notification group.
    pub account_label: String,
    /// How many new messages arrived; may exceed `messages.len()`, which is capped.
    pub new_count: u32,
    /// The newest few previews, newest first.
    pub messages: Vec<NewMailPreview>,
}

/// The result of one background pass: the new inbound mail per account (accounts with none
/// omitted), and whether the pass hit its time budget.
#[derive(uniffi::Record)]
pub struct BackgroundSyncOutcome {
    /// Accounts that received new inbound Inbox mail this pass.
    pub accounts: Vec<AccountNewMail>,
    /// Whether the sync was cut short by its budget (a partial pass; un-synced accounts
    /// catch up next time).
    pub timed_out: bool,
}

impl From<AppNewMailPreview> for NewMailPreview {
    fn from(preview: AppNewMailPreview) -> Self {
        Self {
            sender: preview.sender,
            sender_name: preview.sender_name,
            subject: preview.subject,
            received: preview.received,
            message_key: preview.message_key,
        }
    }
}

impl From<AppAccountNewMail> for AccountNewMail {
    fn from(account: AppAccountNewMail) -> Self {
        Self {
            account_id: account.account_id,
            account_label: account.account_label,
            new_count: account.new_count,
            messages: account.messages.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AppBackgroundNewMail> for BackgroundSyncOutcome {
    fn from(outcome: AppBackgroundNewMail) -> Self {
        Self {
            accounts: outcome.accounts.into_iter().map(Into::into).collect(),
            timed_out: outcome.timed_out,
        }
    }
}

#[uniffi::export]
impl MailcalApp {
    /// Reports newly-arrived inbound Inbox mail already synced by the desktop live runtime,
    /// without starting another network pass. A desktop host calls this after a mailbox-surface
    /// change and posts the returned local notifications. The same persisted high-water marks as
    /// [`run_background_sync`](Self::run_background_sync) provide first-run seeding and dedupe.
    pub fn collect_cached_new_mail(&self) -> BackgroundSyncOutcome {
        self.runtime
            .block_on(self.app.collect_cached_new_mail())
            .into()
    }

    /// Builds a **headless** core for a background-sync worker: connects every stored account
    /// and opens the store (like [`new_accounts`](Self::new_accounts)) but does **not** start
    /// the standing IMAP IDLE watches / poll timers. A cold OS worker (Android WorkManager /
    /// iOS `BGAppRefreshTask` after the app was killed) constructs this, calls
    /// [`run_background_sync`](Self::run_background_sync) once, and lets the process suspend; a
    /// warm app reuses its live instance instead.
    ///
    /// A background pass refreshes access tokens exactly like a foreground one, so it can be
    /// handed a **rotated refresh token**, and this core is dropped at the end of the pass, with
    /// no later moment in which to save one. That is why `credential_store` is a parameter here,
    /// as it is on [`new_accounts`](Self::new_accounts): both mobile hosts once built this core
    /// and then called none of the three setters it used to have, so every rotation in a cold
    /// background pass was silently dropped for as long as the worker existed. See
    /// [`crate::credential_store`].
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError`] if the runtime cannot start or the engine cannot open. A single
    /// account's connect failure is non-fatal; it is kept as a placeholder, as at normal boot.
    #[allow(clippy::too_many_arguments)]
    #[uniffi::constructor]
    pub fn new_background_worker(
        observer: Box<dyn Observer>,
        logger: Box<dyn Logger>,
        log_level: LogLevel,
        configs: Vec<String>,
        data_dir: String,
        device_timezone: String,
        device_info: DeviceInfo,
        credential_store: Box<dyn crate::AccountCredentialStore>,
    ) -> Result<Arc<Self>, MailcalError> {
        boot::build_accounts(
            boot::HostPorts {
                observer,
                logger,
                credential_store: Arc::from(credential_store),
            },
            log_level,
            configs,
            data_dir,
            boot::HostDevice {
                timezone: device_timezone,
                info: device_info,
            },
            boot::BootMode {
                // Headless: no standing IDLE/poll runtime; one bounded pass, then quiesce.
                start_live_sync: false,
            },
        )
    }

    /// Runs one **bounded** sync pass across every account and returns the newly-arrived
    /// inbound Inbox mail since the previous pass, for the host to raise a local notification
    /// per message. Blocks until the pass finishes or `budget_seconds` elapses (clamped to a
    /// sane band), so the host can then post its notifications and mark its OS task complete.
    ///
    /// Reuses the same refresh the live runtime does, so it is safe whether the app is warm
    /// (its live watches/poll are up) or a cold headless worker. See `docs/background-sync.md`.
    pub fn run_background_sync(&self, budget_seconds: u32) -> BackgroundSyncOutcome {
        let budget =
            Duration::from_secs(u64::from(budget_seconds).clamp(MIN_BUDGET_SECS, MAX_BUDGET_SECS));
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(async move { app.run_background_sync(budget).await })
            .into()
    }
}
