//! The mail list's bottom strip: what this client says about mail arriving, and about it not
//! arriving.
//!
//! Three surfaces, kept apart on purpose (`docs/sync-progress.md`): the **bar** for a download
//! the user is waiting on, which is the only one allowed a row of layout; the **hint** for a
//! pass nobody started; and the **pause** for an account whose server asked to be left alone for
//! a while. Split out of `model.rs` to keep both files under the size limit.

use mailcal_bindings::{AccountRow, SyncProgressSnapshot};

use crate::l10n;

/// One awaited mail download, ready for the strip under the message list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SyncBar {
    pub(crate) caption: String,
    /// `None` keeps the bar indeterminate until the provider reports a total.
    pub(crate) fraction: Option<f64>,
}

/// What the mail list's bottom bar says, and the sentence behind its tooltip where it has one.
///
/// `None` whenever there is nothing to say, which is almost always.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyncStatus {
    /// The caption itself.
    pub(crate) text: String,
    /// The whole story, for the tooltip, where [`text`](Self::text) is a short label standing
    /// in for one. `None` where the caption already says everything.
    pub(crate) detail: Option<String>,
}

/// The mail list's bottom-bar caption: an account being made to wait, or the accounts pulling
/// mail down right now and how far through their folders they are.
///
/// A caption, never a bar: a pass the user did not start may not take a row of layout and move
/// the list.
///
/// **A pause takes the line ahead of the hint.** A pass that is downloading is already evident
/// from the list filling; a pass that is waiting is evident from nothing at all, which is the
/// question this answers. The core keeps the two sets disjoint, so this only has to order them.
pub(crate) fn sync_status(
    progress: &SyncProgressSnapshot,
    accounts: &[AccountRow],
) -> Option<SyncStatus> {
    if let Some(paused) = sync_pause(progress, accounts) {
        return Some(paused);
    }
    Some(SyncStatus {
        text: sync_hint(progress, accounts)?,
        detail: None,
    })
}

/// The paused notice: a server answered promptly and asked to be left alone for a while.
///
/// A short label with the sentence behind a hover, because this is a desktop with a pointer and
/// the line is shared with the folder counts. The wait is the server's own, so it is stated as
/// an approximation: it was rounded up before it got here, and nothing re-reads the clock while
/// the tooltip is up.
fn sync_pause(progress: &SyncProgressSnapshot, accounts: &[AccountRow]) -> Option<SyncStatus> {
    let only = progress.throttled.first()?;
    // Several at once are not named, exactly as with the hint: a status line cannot name them
    // all, and their waits have no shared end to state.
    let detail = if progress.throttled.len() > 1 {
        let count = i64::try_from(progress.throttled.len()).unwrap_or(i64::MAX);
        l10n::sync_paused_detail_accounts(count)
    } else {
        let name = account_name(accounts, &only.account_id);
        match only.resumes_in_minutes {
            Some(minutes) => l10n::sync_paused_detail(name, i64::from(minutes)),
            // Two refusals in three name no instant. Saying "shortly" is the honest answer;
            // a figure here would be one we made up.
            None => l10n::sync_paused_detail_soon(name),
        }
    };
    Some(SyncStatus {
        text: l10n::sync_paused().to_owned(),
        detail: Some(detail),
    })
}

/// Names an account from the app's own account list, which is where every other surface gets
/// the address; the id is the fallback for one removed mid-pass.
fn account_name<'a>(accounts: &'a [AccountRow], id: &'a str) -> &'a str {
    accounts
        .iter()
        .find(|row| row.id == id)
        .map_or(id, |row| row.email.as_str())
}

/// The background-sync hint: which accounts are pulling mail down right now, and how far
/// through their folders they are.
///
/// `None` whenever nothing is arriving unasked, which is almost always: the core admits an
/// account only once its background pass has actually committed mail, so a poll that finds
/// nothing renders nothing.
fn sync_hint(progress: &SyncProgressSnapshot, accounts: &[AccountRow]) -> Option<String> {
    let only = progress.accounts.first()?;
    // Several at once carry no counts: one account in its folders and another in its bodies have
    // no shared unit to add up, and a status line cannot name them all anyway.
    if progress.accounts.len() > 1 {
        let count = i64::try_from(progress.accounts.len()).unwrap_or(i64::MAX);
        return Some(l10n::sync_hint_accounts(count));
    }
    let name = account_name(accounts, &only.account_id);
    if only.warming_bodies {
        return Some(l10n::sync_hint_bodies(name, &only.bodies_done.to_string()));
    }
    Some(l10n::sync_hint_account(
        name,
        &only.folders_done.to_string(),
        &only.folders_total.to_string(),
    ))
}

/// The foreground-download bar. A background pass never reaches this projection.
#[allow(clippy::cast_precision_loss)]
// GTK accepts only `f64` progress fractions. Message counts above f64's exact integer range are
// already far beyond a usable progress total; the caption keeps the original integer values.
pub(crate) fn sync_bar(progress: &SyncProgressSnapshot) -> Option<SyncBar> {
    if !progress.active {
        return None;
    }
    let fetched = sync_count(progress.fetched);
    let (caption, fraction) = progress.total.map_or_else(
        || (l10n::sync_downloading_indeterminate(&fetched), None),
        |total| {
            let fraction =
                (total > 0).then(|| (progress.fetched as f64 / total as f64).clamp(0.0, 1.0));
            (
                l10n::sync_downloading(&fetched, &sync_count(total)),
                fraction,
            )
        },
    );
    Some(SyncBar { caption, fraction })
}

fn sync_count(value: u64) -> String {
    let separator = match l10n::active_locale() {
        "fr" => '\u{202f}',
        "de" | "es" | "it" | "nl" | "pt" => '.',
        _ => ',',
    };
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(separator);
        }
        grouped.push(digit);
    }
    grouped
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{AccountRow, SyncProgressSnapshot, ThrottledAccount};

    use super::{sync_bar, sync_status};

    /// A snapshot with nothing but the accounts a server has asked to wait.
    fn paused(accounts: Vec<ThrottledAccount>) -> SyncProgressSnapshot {
        SyncProgressSnapshot {
            active: false,
            fetched: 0,
            total: None,
            accounts: Vec::new(),
            throttled: accounts,
        }
    }

    fn waiting(id: &str, minutes: Option<u32>) -> ThrottledAccount {
        ThrottledAccount {
            account_id: id.to_owned(),
            resumes_in_minutes: minutes,
        }
    }

    fn account_row(id: &str, email: &str) -> AccountRow {
        AccountRow {
            id: id.to_owned(),
            email: email.to_owned(),
            name: String::new(),
            expanded: false,
        }
    }

    #[test]
    fn an_awaited_download_projects_a_determinate_bar_and_localized_counts() {
        let bar = sync_bar(&SyncProgressSnapshot {
            active: true,
            fetched: 1_200,
            total: Some(3_387),
            accounts: Vec::new(),
            throttled: Vec::new(),
        })
        .expect("an active awaited download has a bar");

        assert_eq!(bar.caption, "Downloading 1,200 of 3,387…");
        assert_eq!(bar.fraction, Some(1_200.0 / 3_387.0));
    }

    #[test]
    fn an_unknown_total_is_indeterminate_and_an_idle_download_has_no_bar() {
        let bar = sync_bar(&SyncProgressSnapshot {
            active: true,
            fetched: 1_200,
            total: None,
            accounts: Vec::new(),
            throttled: Vec::new(),
        })
        .expect("an active download without a total still has a bar");
        assert_eq!(bar.caption, "Downloading 1,200…");
        assert_eq!(bar.fraction, None);

        assert!(
            sync_bar(&SyncProgressSnapshot {
                active: false,
                fetched: 0,
                total: None,
                accounts: Vec::new(),
                throttled: Vec::new(),
            })
            .is_none()
        );
    }

    #[test]
    fn a_paused_account_is_a_short_label_with_the_wait_behind_a_hover() {
        // The desktop shape: the line is shared with the folder counts and the message total,
        // so the sentence goes in the tooltip rather than crowding them out. The wait is the
        // server's own, stated as an approximation because nothing re-reads the clock while
        // the tooltip is up.
        let status = sync_status(
            &paused(vec![waiting("a", Some(5))]),
            &[account_row("a", "eva@example.com")],
        )
        .expect("a paused account has something to say");
        assert_eq!(status.text, "Sync paused");
        assert_eq!(
            status.detail.as_deref(),
            Some(
                "The mail server for eva@example.com asked us to slow down. Syncing continues \
                 in about 5 minutes."
            ),
        );
    }

    #[test]
    fn a_pause_the_server_did_not_time_says_so_rather_than_inventing_a_figure() {
        // Two refusals in three name no instant, so this is the commoner of the two shapes and
        // the one a host is likeliest to render wrong.
        let status = sync_status(&paused(vec![waiting("a", None)]), &[]).expect("still a pause");
        assert_eq!(
            status.detail.as_deref(),
            Some("The mail server for a asked us to slow down. Syncing continues shortly."),
            "and falls back to the id for an account the list no longer holds",
        );
    }

    #[test]
    fn a_pause_takes_the_line_ahead_of_the_hint() {
        // A pass that is downloading is already evident from the list filling; a pass that is
        // waiting is evident from nothing at all, which is the question the line answers.
        let mut progress = paused(vec![waiting("b", Some(2))]);
        progress.accounts = vec![mailcal_bindings::AccountSyncProgress {
            account_id: "a".to_owned(),
            folders_done: 3,
            folders_total: 12,
            warming_bodies: false,
            bodies_done: 0,
        }];
        let status = sync_status(&progress, &[account_row("a", "eva@example.com")])
            .expect("there is something to say");
        assert_eq!(status.text, "Sync paused");
    }

    #[test]
    fn a_quiet_client_says_nothing_at_all() {
        assert!(sync_status(&paused(Vec::new()), &[]).is_none());
    }
}
