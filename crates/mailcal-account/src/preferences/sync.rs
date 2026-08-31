//! Mail synchronisation settings; how far back to sync ([`SyncDepth`]) and how new mail
//! arrives (push vs. poll, [`SyncStrategy`] / [`AccountSyncSettings`]), plus resolving a
//! stored choice against whether the server advertises `IDLE` ([`effective`]).
//!
//! These are the sync-behaviour half of the app's [`Preferences`](super::Preferences): the
//! product-core owns the types and the resolution rules; the engine only acts on the values
//! (the [`SyncDepth`] cutoff used to build a per-sync window).

use serde::{Deserialize, Serialize};
use time::Date;

/// How far back to sync mail: the **app-level sync-depth** setting. The default is the
/// last three months; a user can widen it (6 / 9 / 12 / 24 months) or sync **all** mail.
///
/// Serialized as a plain month count (`0` = [`AllTime`](SyncDepth::AllTime)), so the
/// preferences file stays `sync_depth = 3` rather than a tagged enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "u16", into = "u16")]
pub enum SyncDepth {
    /// Sync only mail delivered within the last N months.
    Months(u16),
    /// Sync the entire mailbox, with no date cutoff.
    AllTime,
}

impl Default for SyncDepth {
    /// The shipping default across every platform: the last three months.
    fn default() -> Self {
        Self::Months(3)
    }
}

impl SyncDepth {
    /// The earliest date to sync mail from, given the device's `today`, or `None` for
    /// [`AllTime`](SyncDepth::AllTime). Subtracts whole **calendar months** (not 30-day
    /// spans), clamping the day to the target month's length (e.g. Mar 31 − 1 month →
    /// Feb 28). The result is the `time::Date` a host turns into a per-sync window.
    #[must_use]
    pub fn cutoff(self, today: Date) -> Option<Date> {
        let months = match self {
            Self::AllTime => return None,
            Self::Months(m) => i32::from(m),
        };
        // Work in absolute months since year 0 so the subtraction borrows across years.
        let month_index = i32::from(u8::from(today.month())) - 1; // 0..=11
        let total = today.year() * 12 + month_index - months;
        let year = total.div_euclid(12);
        let month = time::Month::try_from(u8::try_from(total.rem_euclid(12) + 1).unwrap_or(1))
            .unwrap_or(time::Month::January);
        let day = today.day().min(month.length(year));
        Date::from_calendar_date(year, month, day).ok()
    }
}

impl From<u16> for SyncDepth {
    fn from(months: u16) -> Self {
        if months == 0 {
            Self::AllTime
        } else {
            Self::Months(months)
        }
    }
}

impl From<SyncDepth> for u16 {
    fn from(depth: SyncDepth) -> Self {
        match depth {
            SyncDepth::Months(months) => months,
            SyncDepth::AllTime => 0,
        }
    }
}

/// The selectable mail sync-depth options, in display order, as month counts; `0` is the
/// **all mail** sentinel ([`SyncDepth::AllTime`]). A client builds its per-account fetch-depth
/// picker from this (mapping each value through [`SyncDepth::from`]) rather than hardcoding the
/// set, so it stays defined in one place across platforms.
pub const SYNC_DEPTHS: [u16; 6] = [3, 6, 9, 12, 24, 0];

/// How big a message this account downloads in full during the background body warm; the
/// **message-size** setting.
///
/// A warm fetches whole raw sources, so what a pass costs is bytes rather than messages. Above
/// the cap the body waits for the open that asks for it, which fetches and caches it then.
///
/// Serialized as a plain megabyte count (`0` = [`Unlimited`](MessageSizeLimit::Unlimited)), the
/// same shape as [`SyncDepth`], so the preferences file stays `message_size_limit = 2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "u16", into = "u16")]
pub enum MessageSizeLimit {
    /// Warm only messages at or under N megabytes.
    Megabytes(u16),
    /// Warm every message, whatever it costs on a metered link.
    Unlimited,
}

impl MessageSizeLimit {
    /// The cap in octets, or `None` for [`Unlimited`](MessageSizeLimit::Unlimited): the form
    /// the warm and the reclaim both compare against.
    #[must_use]
    pub const fn octets(self) -> Option<u64> {
        match self {
            Self::Unlimited => None,
            Self::Megabytes(mb) => Some(mb as u64 * 1024 * 1024),
        }
    }
}

impl From<u16> for MessageSizeLimit {
    fn from(megabytes: u16) -> Self {
        if megabytes == 0 {
            Self::Unlimited
        } else {
            Self::Megabytes(megabytes)
        }
    }
}

impl From<MessageSizeLimit> for u16 {
    fn from(limit: MessageSizeLimit) -> Self {
        match limit {
            MessageSizeLimit::Megabytes(mb) => mb,
            MessageSizeLimit::Unlimited => 0,
        }
    }
}

/// The selectable message-size caps, in display order, as megabyte counts; `0` is the
/// **no limit** sentinel ([`MessageSizeLimit::Unlimited`]). A client builds its per-account
/// picker from this rather than hardcoding the set, so it stays defined in one place across
/// platforms.
pub const MESSAGE_SIZE_LIMITS_MB: [u16; 4] = [2, 5, 10, 0];

/// The most folders an account may subscribe to for IMAP `IDLE` push: the same cap on
/// every platform (the desktop watch is cheap, but the limit bounds the mobile battery
/// cost of one standing connection per watched folder, and keeping it uniform means one
/// rule across clients). The product core enforces it; clients also disable further
/// selection once it is reached (`AccountSyncRow::at_push_limit`).
pub const MAX_PUSH_FOLDERS: usize = 5;

/// The selectable background-poll intervals, in minutes; what a client offers when an
/// account checks on a timer instead of receiving push. 15 minutes is the floor (it
/// matches Android `WorkManager`'s minimum periodic interval).
pub const POLL_INTERVALS: [u16; 5] = [15, 30, 60, 90, 120];

/// The default poll interval (30 minutes) for an account that polls; used when no
/// interval is stored, or to repair an out-of-range stored value.
pub const DEFAULT_POLL_INTERVAL: u16 = 30;

/// How an account receives new mail. The companion folder set / interval live alongside
/// it in [`AccountSyncSettings`], which one is *in effect* also depends on whether the
/// server advertises `IDLE` (see [`effective`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStrategy {
    /// Receive mail as it arrives, via a standing IMAP `IDLE` connection per subscribed
    /// folder. Valid only when the server advertises `IDLE`.
    Push,
    /// Check for new mail on a timer (`poll_interval_mins`).
    Poll,
}

/// One account's persisted synchronisation-behaviour choice. Absent for an account the
/// user hasn't customised; [`effective`] then derives the shipping default (push the
/// Inbox where `IDLE` is supported, else poll every [`DEFAULT_POLL_INTERVAL`] minutes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSyncSettings {
    /// Whether the account pushes or polls.
    pub strategy: SyncStrategy,
    /// The folders watched for push (provider keys), capped at [`MAX_PUSH_FOLDERS`].
    /// Empty falls back to the Inbox at resolution time.
    #[serde(default)]
    pub push_folders: Vec<String>,
    /// The poll interval in minutes; one of [`POLL_INTERVALS`].
    #[serde(default = "default_poll_interval")]
    pub poll_interval_mins: u16,
    /// How far back to sync **this account's** mail. `None` uses the product default
    /// ([`SyncDepth::default`], currently three months): the state every account starts in.
    /// `Some` is an explicit per-account override the Settings screen sets.
    #[serde(default)]
    pub sync_depth: Option<SyncDepth>,
    /// The largest message the body warm pulls in full for **this account**. `None` uses the
    /// product default, which differs by device: the state every account starts in. `Some` is
    /// an explicit per-account override the Settings screen sets.
    ///
    /// There is no `Default` here on purpose: what the default *is* depends on the kind of
    /// device, which this crate does not know. The app layer resolves `None`.
    #[serde(default)]
    pub message_size_limit: Option<MessageSizeLimit>,
}

/// serde default for [`AccountSyncSettings::poll_interval_mins`].
fn default_poll_interval() -> u16 {
    DEFAULT_POLL_INTERVAL
}

/// The synchronisation behaviour actually in effect for an account, after resolving the
/// stored choice (if any) against whether the server supports `IDLE`. Push that the
/// server can't honour degrades to polling; never silently off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSync {
    /// The strategy in effect (push degrades to poll without `IDLE`).
    pub strategy: SyncStrategy,
    /// The folders that would be watched under push; deduped and capped at
    /// [`MAX_PUSH_FOLDERS`]; defaulted to `default_push_folders` when none are stored.
    pub push_folders: Vec<String>,
    /// The interval in effect under poll; snapped to [`POLL_INTERVALS`].
    pub poll_interval_mins: u16,
}

/// Dedupes `folders` (preserving order) and truncates to [`MAX_PUSH_FOLDERS`].
#[must_use]
pub fn cap_push_folders(folders: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    folders
        .iter()
        .filter(|folder| seen.insert((*folder).clone()))
        .take(MAX_PUSH_FOLDERS)
        .cloned()
        .collect()
}

/// Snaps `minutes` to the nearest allowed [`POLL_INTERVALS`] value, falling back to
/// [`DEFAULT_POLL_INTERVAL`] when it is zero/unset: so a malformed or legacy stored value
/// can never produce a busy-loop or an unbounded interval.
#[must_use]
pub fn snap_poll_interval(minutes: u16) -> u16 {
    if minutes == 0 {
        return DEFAULT_POLL_INTERVAL;
    }
    POLL_INTERVALS
        .into_iter()
        .min_by_key(|allowed| allowed.abs_diff(minutes))
        .unwrap_or(DEFAULT_POLL_INTERVAL)
}

/// Resolves the synchronisation behaviour **in effect** for an account from its `stored`
/// choice (if any) and whether the server advertises `IDLE`:
///
/// - No stored choice → the shipping default: push `default_push_folders` (the Inbox) when
///   `idle_supported`, else poll every [`DEFAULT_POLL_INTERVAL`] minutes.
/// - Stored push, but the server lacks `IDLE` → degrade to polling (the stored interval, or the
///   default); push is never silently dropped to nothing.
/// - Stored push with `IDLE` → push the stored folders (Inbox-defaulted if empty), capped.
/// - Stored poll → poll the stored interval (snapped to the allowed set).
#[must_use]
pub fn effective(
    stored: Option<&AccountSyncSettings>,
    idle_supported: bool,
    default_push_folders: &[String],
) -> EffectiveSync {
    let poll = |mins: u16| EffectiveSync {
        strategy: SyncStrategy::Poll,
        push_folders: Vec::new(),
        poll_interval_mins: snap_poll_interval(mins),
    };
    match stored {
        None if idle_supported => EffectiveSync {
            strategy: SyncStrategy::Push,
            push_folders: cap_push_folders(default_push_folders),
            poll_interval_mins: DEFAULT_POLL_INTERVAL,
        },
        None => poll(DEFAULT_POLL_INTERVAL),
        Some(settings) => match settings.strategy {
            // Stored push the server can't honour degrades to polling, not silence.
            SyncStrategy::Push if !idle_supported => poll(settings.poll_interval_mins),
            SyncStrategy::Push => {
                let folders = if settings.push_folders.is_empty() {
                    cap_push_folders(default_push_folders)
                } else {
                    cap_push_folders(&settings.push_folders)
                };
                EffectiveSync {
                    strategy: SyncStrategy::Push,
                    push_folders: folders,
                    poll_interval_mins: snap_poll_interval(settings.poll_interval_mins),
                }
            }
            SyncStrategy::Poll => poll(settings.poll_interval_mins),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_depths_options_map_through_syncdepth_with_zero_as_all_time() {
        // The picker list is month counts with `0` = all mail, and each maps cleanly.
        assert_eq!(SYNC_DEPTHS, [3, 6, 9, 12, 24, 0]);
        assert_eq!(SyncDepth::from(SYNC_DEPTHS[0]), SyncDepth::Months(3));
        assert_eq!(
            SyncDepth::from(*SYNC_DEPTHS.last().unwrap()),
            SyncDepth::AllTime
        );
    }

    #[test]
    fn sync_depth_serializes_as_a_month_count_with_zero_for_all_time() {
        // The TOML stays a plain integer; 0 is the All-time sentinel.
        assert_eq!(u16::from(SyncDepth::Months(6)), 6);
        assert_eq!(u16::from(SyncDepth::AllTime), 0);
        assert_eq!(SyncDepth::from(9), SyncDepth::Months(9));
        assert_eq!(SyncDepth::from(0), SyncDepth::AllTime);
    }

    #[test]
    fn cutoff_subtracts_whole_calendar_months() {
        let today = Date::from_calendar_date(2026, time::Month::June, 27).unwrap();
        assert_eq!(
            SyncDepth::Months(3).cutoff(today),
            Some(Date::from_calendar_date(2026, time::Month::March, 27).unwrap())
        );
        // Crossing a year boundary borrows correctly.
        assert_eq!(
            SyncDepth::Months(12).cutoff(today),
            Some(Date::from_calendar_date(2025, time::Month::June, 27).unwrap())
        );
        // All-time has no cutoff.
        assert_eq!(SyncDepth::AllTime.cutoff(today), None);
    }

    #[test]
    fn cutoff_clamps_the_day_to_a_shorter_target_month() {
        // Mar 31 − 1 month lands in February, which has no 31st (28 days in 2026).
        let mar31 = Date::from_calendar_date(2026, time::Month::March, 31).unwrap();
        assert_eq!(
            SyncDepth::Months(1).cutoff(mar31),
            Some(Date::from_calendar_date(2026, time::Month::February, 28).unwrap())
        );
    }

    #[test]
    fn cap_push_folders_dedupes_and_truncates_to_five() {
        let many = ["INBOX", "A", "B", "INBOX", "C", "D", "E", "F"].map(str::to_owned);
        let capped = cap_push_folders(&many);
        // Deduped (one INBOX) and never more than five, in first-seen order.
        assert_eq!(capped, ["INBOX", "A", "B", "C", "D"].map(str::to_owned));
        assert_eq!(capped.len(), MAX_PUSH_FOLDERS);
    }

    #[test]
    fn snap_poll_interval_repairs_out_of_range_values() {
        // An allowed value is kept; an odd one snaps to the nearest; zero/unset → default.
        assert_eq!(snap_poll_interval(60), 60);
        assert_eq!(snap_poll_interval(20), 15);
        assert_eq!(snap_poll_interval(1000), 120);
        assert_eq!(snap_poll_interval(0), DEFAULT_POLL_INTERVAL);
    }

    #[test]
    fn effective_default_pushes_inbox_when_idle_supported_else_polls() {
        let inbox = ["INBOX".to_owned()];
        // No stored choice + IDLE → push the Inbox.
        let push = effective(None, true, &inbox);
        assert_eq!(push.strategy, SyncStrategy::Push);
        assert_eq!(push.push_folders, inbox);
        // No stored choice + no IDLE → poll at the default interval.
        let poll = effective(None, false, &inbox);
        assert_eq!(poll.strategy, SyncStrategy::Poll);
        assert_eq!(poll.poll_interval_mins, DEFAULT_POLL_INTERVAL);
    }

    #[test]
    fn effective_degrades_stored_push_to_poll_without_idle() {
        // A server that dropped IDLE (or never had it) must not leave a push account silent.
        let stored = AccountSyncSettings {
            strategy: SyncStrategy::Push,
            push_folders: vec!["INBOX".to_owned()],
            poll_interval_mins: 90,
            sync_depth: None,
            message_size_limit: None,
        };
        let eff = effective(Some(&stored), false, &["INBOX".to_owned()]);
        assert_eq!(eff.strategy, SyncStrategy::Poll);
        // The stored interval is honoured on the fallback (snapped to the allowed set).
        assert_eq!(eff.poll_interval_mins, 90);
        assert!(eff.push_folders.is_empty());
    }

    #[test]
    fn effective_push_defaults_empty_folder_set_to_the_inbox() {
        // Stored push with no folders falls back to the Inbox rather than watching nothing.
        let stored = AccountSyncSettings {
            strategy: SyncStrategy::Push,
            push_folders: Vec::new(),
            poll_interval_mins: DEFAULT_POLL_INTERVAL,
            sync_depth: None,
            message_size_limit: None,
        };
        let eff = effective(Some(&stored), true, &["INBOX".to_owned()]);
        assert_eq!(eff.strategy, SyncStrategy::Push);
        assert_eq!(eff.push_folders, ["INBOX".to_owned()]);
    }
}
