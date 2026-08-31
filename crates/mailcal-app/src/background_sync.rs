//! One-shot background sync: a bounded pass across every account that reports the
//! **newly-arrived inbound Inbox mail** since the previous run, for a host to raise local
//! notifications from.
//!
//! On Android and iOS the OS suspends the process shortly after it leaves the foreground,
//! freezing the live IDLE/poll runtime (`crate::sync` + the bindings' `background` manager).
//! A host therefore schedules this from its OS background mechanism: a WorkManager worker
//! (Android) or a `BGAppRefreshTask` (iOS); to catch up while backgrounded. It reuses
//! [`App::refresh_mail`] for the sync itself and the snapshot's own message cache for
//! detection, so it needs no extra store I/O and no engine change.
//!
//! Dedupe is a **persisted per-account high-water-mark** (the newest inbound-Inbox instant
//! already reported), kept in the shared `preferences.toml`. The first run per account seeds
//! the mark and reports nothing, so enabling the feature never notifies the existing inbox.
//! This same entry point is what a future push handler will call (`docs/background-sync.md`).

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use engine_api::{MailListRow, UtcDateTime};
use mailcal_account::{load_preferences, save_preferences};
use time::OffsetDateTime;

use crate::{App, Event, Feature, snapshot::is_outgoing, sync::RefreshProgress};

/// How many message previews one account contributes to the result, at most. A host raises one
/// notification per preview, then a summary for any messages beyond this cap;
/// [`AccountNewMail::new_count`] carries the true total needed for that summary.
const MAX_PREVIEWS: usize = 5;

/// One newly-arrived inbound message, projected for a host notification.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// The message's provider key; its stable identity, so a host can dedupe the OS
    /// notification and deep-link a tap to the message (the key mailbox rows open by).
    pub message_key: String,
}

/// The new inbound Inbox mail one account received during a background pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountNewMail {
    /// The id of the account the mail arrived on.
    pub account_id: String,
    /// The account's address (its login identity), for a per-account notification group.
    pub account_label: String,
    /// How many new messages arrived; may exceed `messages.len()`, which is capped.
    pub new_count: u32,
    /// The newest few previews (capped), newest first.
    pub messages: Vec<NewMailPreview>,
}

/// The result of one background pass: the new inbound mail per account (accounts with none
/// omitted), and whether the sync was cut short by its time budget.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackgroundNewMail {
    /// Accounts that received new inbound Inbox mail this pass.
    pub accounts: Vec<AccountNewMail>,
    /// Whether the sync pass hit its budget before finishing (some accounts may not have
    /// synced this pass; their mark is untouched, so they catch up next time).
    pub timed_out: bool,
}

impl<P: engine_api::Provider> App<P> {
    /// Reports newly-arrived inbound Inbox mail already present in the cache, without starting a
    /// network refresh. Desktop hosts call this after their live IDLE/poll runtime publishes a
    /// mailbox change, so notifications inherit the user's configured sync cadence rather than
    /// introducing a second timer. Like [`Self::run_background_sync`], the first call seeds the
    /// persisted high-water marks and reports nothing.
    pub async fn collect_cached_new_mail(&self) -> BackgroundNewMail {
        BackgroundNewMail {
            accounts: self.collect_new_inbound().await,
            timed_out: false,
        }
    }

    /// Runs one **bounded** sync pass across every account, then reports the newly-arrived
    /// inbound Inbox mail since the previous pass so the host can raise a local notification
    /// per message.
    ///
    /// Reuses `refresh_mail` (which honours per-account push/poll settings, the sync-depth window,
    /// and offline gating) under a `budget` timeout. Safe whether or not the live sync runtime is
    /// up: `refresh_mail` drops the accounts read-guard before any
    /// network round-trip and per-scope contention with a live poll is absorbed as a skipped
    /// (`Busy`) sync, so a warm app's poll and this one-shot coexist.
    pub async fn run_background_sync(&self, budget: Duration) -> BackgroundNewMail {
        // Counts that background delivery ran at all: the adoption signal for the feature. The
        // per-account sync events `refresh_mail` emits carry its health.
        self.track(Event::FeatureUsed {
            feature: Feature::BackgroundSync,
        });
        // On timeout still report whatever synced before the deadline; better than dropping
        // fetched mail, and flag the partial pass.
        let timed_out =
            tokio::time::timeout(budget, self.refresh_mail(RefreshProgress::Background))
                .await
                .is_err();
        let accounts = self.collect_new_inbound().await;
        BackgroundNewMail {
            accounts,
            timed_out,
        }
    }

    /// Scans each account's freshly-synced Inbox for messages newer than its persisted
    /// high-water-mark, advances the mark, and returns the previews. The first run per account
    /// (no mark) seeds the mark to the newest existing message (or "now" over an empty inbox)
    /// and reports nothing, so enabling notifications never floods the existing inbox.
    async fn collect_new_inbound(&self) -> Vec<AccountNewMail> {
        let window = self.load_window();
        let mut out = Vec::new();
        // Marks are advanced in memory during the loop and persisted **once** at the end, so a
        // multi-account pass (and every first run, which seeds every account) rewrites the shared
        // `preferences.toml` a single time instead of once per account.
        let mut marks_changed = false;
        for account in self.account_handles().await {
            let id = &account.id;
            let owner = account.identity.email.as_str();
            let Some(inbox) = self.inbox_key(id).await else {
                continue;
            };
            // Read straight from the store rather than through the shown list's cache: a
            // background pass must not replace what the UI is holding, and the read is one
            // indexed query.
            let rows = self
                .engine
                .mail_window(core::slice::from_ref(id), window)
                .await
                .unwrap_or_default();
            let mark = self.mark_for(id.as_str());
            let scan = newly_arrived(&rows, &inbox, Some(owner), mark);
            if mark.is_none() {
                // First run: seed and report nothing.
                self.set_mark(id.as_str(), scan.high_water.unwrap_or_else(now_utc));
                marks_changed = true;
            } else {
                // Established mark: report only strictly-newer mail, advancing the mark to the
                // newest reported message (so nothing is ever reported twice).
                if scan.previews.is_empty() {
                    continue;
                }
                if let Some(high) = scan.high_water {
                    self.set_mark(id.as_str(), high);
                    marks_changed = true;
                }
                let new_count = u32::try_from(scan.previews.len()).unwrap_or(u32::MAX);
                out.push(AccountNewMail {
                    account_id: id.as_str().to_owned(),
                    account_label: owner.to_owned(),
                    new_count,
                    messages: scan.previews.into_iter().take(MAX_PREVIEWS).collect(),
                });
            }
        }
        if marks_changed {
            self.persist_marks();
        }
        out
    }

    /// The account's stored high-water-mark, if any.
    fn mark_for(&self, id: &str) -> Option<UtcDateTime> {
        self.notify_marks
            .lock()
            .expect("notify-marks mutex poisoned")
            .get(id)
    }

    /// Advances the account's high-water-mark **in memory**. Call
    /// [`persist_marks`](Self::persist_marks) once after a pass to write the batch.
    fn set_mark(&self, id: &str, mark: UtcDateTime) {
        self.notify_marks
            .lock()
            .expect("notify-marks mutex poisoned")
            .set(id, mark);
    }

    /// Writes all advanced marks to the preferences file in a single read-modify-write.
    fn persist_marks(&self) {
        self.notify_marks
            .lock()
            .expect("notify-marks mutex poisoned")
            .persist();
    }
}

/// The outcome of scanning one account's windowed messages: the new inbound-Inbox previews
/// (newest first; empty when `mark` is `None`) and the newest inbound-Inbox instant overall
/// (`high_water`, `None` when the inbox has no dated mail): the value to seed/advance the mark to.
struct NewMailScan {
    previews: Vec<NewMailPreview>,
    high_water: Option<UtcDateTime>,
}

/// Finds the `candidates` that are inbound (the `owner` did not send them), belong to the
/// account's `inbox`, and were received strictly after `mark`. Newest first. Undated messages
/// are skipped; they can't be ordered against the mark. With `mark` `None` (a first run) no
/// previews are returned, only `high_water`, so the caller seeds the mark without notifying
/// the existing inbox.
fn newly_arrived(
    candidates: &[MailListRow],
    inbox: &str,
    owner: Option<&str>,
    mark: Option<UtcDateTime>,
) -> NewMailScan {
    let mut inbound: Vec<&MailListRow> = candidates
        .iter()
        .filter(|row| {
            !is_outgoing(row, owner)
                && row
                    .mailboxes
                    .iter()
                    .any(|mailbox| mailbox.as_str() == inbox)
        })
        .collect();
    // The row's instant is the delivery date falling back to the `Date` header: the same one the
    // list orders by, so a notification and its row cannot disagree about when mail arrived.
    // Newest first; undated (`None`) sinks last and is excluded from previews anyway.
    inbound.sort_by_key(|row| std::cmp::Reverse(row.mail.date_utc));
    // With the sort above, the first element carries the newest instant (undated sinks last, so a
    // `None` here means every message was undated): no second pass needed.
    let high_water = inbound.first().and_then(|row| row.mail.date_utc);
    let previews = match mark {
        None => Vec::new(),
        Some(mark) => inbound
            .iter()
            .filter(|row| row.mail.date_utc.is_some_and(|received| received > mark))
            .map(|row| preview_of(row))
            .collect(),
    };
    NewMailScan {
        previews,
        high_water,
    }
}

/// Projects one row into a notification preview (sender, subject, received, stable key).
fn preview_of(row: &MailListRow) -> NewMailPreview {
    NewMailPreview {
        sender: row.mail.from_addr.clone().unwrap_or_default(),
        sender_name: row.mail.from_name.clone(),
        subject: row.mail.subject.clone().unwrap_or_default(),
        received: row
            .mail
            .date_utc
            .map(|received| received.to_string())
            .unwrap_or_default(),
        message_key: row.mail.key.as_str().to_owned(),
    }
}

/// The current wall-clock instant as a [`UtcDateTime`] (whole seconds), matching the engine's
/// own `SystemClock`. Used only to seed a first background pass over an **empty** inbox, so the
/// next arrival notifies rather than being swallowed as the seed.
fn now_utc() -> UtcDateTime {
    let now = OffsetDateTime::now_utc();
    UtcDateTime::new(
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
    .expect("a civil UTC time from the system clock is always representable")
}

/// The persisted per-account new-mail high-water-marks. Stored in the shared `preferences.toml`
/// (read-modify-write, preserving sibling settings) exactly like
/// [`SyncSettingsState`](crate::sync_settings); RFC3339 on disk, parsed to [`UtcDateTime`] here.
pub(crate) struct NotifyMarksState {
    marks: BTreeMap<String, UtcDateTime>,
    prefs_path: Option<PathBuf>,
}

impl NotifyMarksState {
    /// Loads the stored marks from the preferences file (dropping any that fail to parse; a
    /// corrupt entry simply re-seeds that account on the next pass).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let marks = prefs_path
            .as_ref()
            .map(|path| load_preferences(path).notify_marks)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, raw)| raw.parse::<UtcDateTime>().ok().map(|mark| (id, mark)))
            .collect();
        Self { marks, prefs_path }
    }

    /// The account's mark, if one is stored.
    fn get(&self, id: &str) -> Option<UtcDateTime> {
        self.marks.get(id).copied()
    }

    /// Advances one account's mark in memory. [`persist`](Self::persist) writes the batch.
    fn set(&mut self, id: &str, mark: UtcDateTime) {
        self.marks.insert(id.to_owned(), mark);
    }

    /// Writes every in-memory mark to the preferences file; read-modify-write, so the sibling
    /// display-zone / sync / quote preferences in the same file are preserved. Best-effort (a
    /// write failure only means the marks aren't persisted; the in-memory values still hold this
    /// session). Call once per pass, not per account, so a multi-account pass rewrites the file
    /// once.
    fn persist(&self) {
        let Some(path) = &self.prefs_path else {
            return;
        };
        let mut prefs = load_preferences(path);
        prefs.notify_marks = self
            .marks
            .iter()
            .map(|(id, mark)| (id.clone(), mark.to_string()))
            .collect();
        let _ = save_preferences(path, &prefs);
    }
}

#[cfg(test)]
#[path = "background_sync_tests.rs"]
mod tests;
