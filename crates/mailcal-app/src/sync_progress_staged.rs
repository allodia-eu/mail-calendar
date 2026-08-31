//! Debug-only staged sync progress: a download and a background hint a UI suite can actually
//! catch, named by `MAILCAL_FAKE_SYNC_PROGRESS` and `MAILCAL_FAKE_SYNC_HINT`.
//!
//! # Why this exists
//!
//! Both surfaces are up only while a pass is fetching mail, which against any local fixture is a
//! fraction of a second; too short for a UI suite to catch, which is why the bar's placement was
//! checked by eye on every platform until it moved. The rules they have to hold to are
//! geometric: the bar occupies its own row between the message list and the footer, so a pass the
//! user never started cannot move a row out from under their pointer; the hint takes no row at
//! all, sitting inside the status line the footer already draws.
//!
//! These substitute **only** the snapshot the host reads. Everything downstream of it is real:
//! the surface signal, the FFI record, the view-model's bound properties, the row the bar lives
//! in and the status line the hint lives in, and the layout pass that places both. A hook that
//! instead set a client's `SyncProgressVisible` would be a mock of the thing under test, and
//! would go on passing after the wiring between core and client was cut. What sits upstream;
//! which passes are awaited, which accounts announce themselves, and how their counts aggregate;
//! is what `sync_progress`'s own tests cover.
//!
//! Debug builds only, like the harness CA trust beside it, so no shipped binary can be talked
//! into reporting a download that is not happening.

// Only the staged hint names it, and that is compiled out of a release build below.
#[cfg(debug_assertions)]
use mailcal_viewmodel::AccountSyncProgress;
use mailcal_viewmodel::SyncProgressSnapshot;

/// The staged snapshot, when either variable names one.
///
/// Accepted forms, anything else ignored with a warning:
///
/// ```text
/// MAILCAL_FAKE_SYNC_PROGRESS=1200/3387   # determinate: 1,200 of 3,387 downloaded
/// MAILCAL_FAKE_SYNC_PROGRESS=1200        # indeterminate: no total reported yet
/// MAILCAL_FAKE_SYNC_HINT=acct-1:3/12     # one account, 3 of its 12 folders done
/// MAILCAL_FAKE_SYNC_HINT=acct-1:3/12,acct-2:0/5
/// ```
#[cfg(debug_assertions)]
pub(crate) fn pretended_progress() -> Option<SyncProgressSnapshot> {
    static STAGED: std::sync::OnceLock<Option<SyncProgressSnapshot>> = std::sync::OnceLock::new();
    STAGED.get_or_init(read_pretended_progress).clone()
}

/// Reads both staged surfaces once, saying either way what it did.
#[cfg(debug_assertions)]
fn read_pretended_progress() -> Option<SyncProgressSnapshot> {
    let bar = std::env::var("MAILCAL_FAKE_SYNC_PROGRESS").ok();
    let hint = std::env::var("MAILCAL_FAKE_SYNC_HINT").ok();
    if bar.is_none() && hint.is_none() {
        return None;
    }
    let mut staged = SyncProgressSnapshot::default();
    if let Some(raw) = &bar {
        match parse_pretended_download(raw) {
            Some(download) => {
                staged.active = true;
                staged.fetched = download.0;
                staged.total = download.1;
            }
            // Warned rather than debugged, on both paths: this tells the user mail is arriving
            // when none is, so a log that records the run has to say the surface was staged, and
            // a value that silently did nothing is how a test comes to prove the opposite of what
            // it claims.
            None => log::warn!(
                "sync_progress: MAILCAL_FAKE_SYNC_PROGRESS={raw} names no download; ignoring it"
            ),
        }
    }
    if let Some(raw) = &hint {
        staged.accounts = parse_pretended_hint(raw);
        if staged.accounts.is_empty() {
            log::warn!("sync_progress: MAILCAL_FAKE_SYNC_HINT={raw} names no account; ignoring it");
        }
    }
    log::warn!(
        "sync_progress: staged; bar {} of {:?}, hint over {} account(s)",
        staged.fetched,
        staged.total,
        staged.accounts.len()
    );
    Some(staged)
}

#[cfg(not(debug_assertions))]
pub(crate) fn pretended_progress() -> Option<SyncProgressSnapshot> {
    None
}

/// Parses a `MAILCAL_FAKE_SYNC_PROGRESS` value into `(fetched, total)`.
///
/// Kept pure so it can be tested: reading the variable is process-global state, and a test that
/// wrote it would decide the outcome of whichever other test happened to be running beside it.
#[cfg(debug_assertions)]
fn parse_pretended_download(raw: &str) -> Option<(u64, Option<u64>)> {
    let raw = raw.trim();
    let (fetched, total) = match raw.split_once('/') {
        Some((fetched, total)) => (fetched, Some(total.trim().parse::<u64>().ok()?)),
        None => (raw, None),
    };
    Some((fetched.trim().parse().ok()?, total))
}

/// Parses a `MAILCAL_FAKE_SYNC_HINT` value, dropping any entry that names no account.
///
/// `<account>:<done>/<total>` stages the folder phase; `<account>:<done>`: no denominator, which
/// is the body warm's real shape; stages the body phase.
#[cfg(debug_assertions)]
fn parse_pretended_hint(raw: &str) -> Vec<AccountSyncProgress> {
    raw.split(',')
        .filter_map(|entry| {
            let (account, counts) = entry.trim().rsplit_once(':')?;
            let account = account.trim();
            if account.is_empty() {
                return None;
            }
            let account_id = account.to_owned();
            let counts = counts.trim();
            let Some((done, total)) = counts.split_once('/') else {
                return Some(AccountSyncProgress {
                    account_id,
                    warming_bodies: true,
                    bodies_done: counts.parse().ok()?,
                    ..AccountSyncProgress::default()
                });
            };
            Some(AccountSyncProgress {
                account_id,
                folders_done: done.trim().parse().ok()?,
                folders_total: total.trim().parse().ok()?,
                ..AccountSyncProgress::default()
            })
        })
        .collect()
}

// Carries the same `cfg` as the code it covers: the hooks are compiled out of a release build on
// purpose, so under `cargo test --release` these would not fail, they would not compile. The
// workspace gate runs debug.
#[cfg(all(test, debug_assertions))]
mod staged_tests {
    use mailcal_viewmodel::AccountSyncProgress;

    #[test]
    fn a_staged_download_reads_as_the_counts_it_names() {
        assert_eq!(
            super::parse_pretended_download("1200/3387"),
            Some((1200, Some(3387)))
        );
    }

    // A count with no denominator is the indeterminate case, not a parse failure: it is what a
    // real pass reports until every in-flight folder has said how much it is bringing.
    #[test]
    fn a_staged_download_without_a_total_is_indeterminate() {
        assert_eq!(super::parse_pretended_download(" 42 "), Some((42, None)));
    }

    // Anything unparseable reports nothing rather than a zero-count download: a staged bar that
    // silently read as "0 of 0" is exactly how a UI test comes to prove the opposite of its name.
    #[test]
    fn an_unparseable_staged_download_is_refused_rather_than_read_as_empty() {
        for raw in ["", "soon", "1200/", "/3387", "-1", "1200/3387/9"] {
            assert_eq!(
                super::parse_pretended_download(raw),
                None,
                "{raw:?} names no download"
            );
        }
    }

    #[test]
    fn a_staged_hint_reads_as_the_accounts_and_folder_counts_it_names() {
        assert_eq!(
            super::parse_pretended_hint("acct-1:3/12, acct-2:0/5"),
            vec![
                AccountSyncProgress {
                    account_id: "acct-1".to_owned(),
                    folders_done: 3,
                    folders_total: 12,
                    ..AccountSyncProgress::default()
                },
                AccountSyncProgress {
                    account_id: "acct-2".to_owned(),
                    folders_done: 0,
                    folders_total: 5,
                    ..AccountSyncProgress::default()
                },
            ]
        );
    }

    // An account id may itself hold a colon, so the counts are split off the *end*.
    #[test]
    fn a_staged_hint_keeps_an_account_id_that_holds_a_colon() {
        assert_eq!(
            super::parse_pretended_hint("imap:me@example.test:1/2"),
            vec![AccountSyncProgress {
                account_id: "imap:me@example.test".to_owned(),
                folders_done: 1,
                folders_total: 2,
                ..AccountSyncProgress::default()
            }]
        );
    }

    // A count with no denominator is the body warm, which genuinely has no total to give.
    #[test]
    fn a_staged_hint_without_a_denominator_stages_the_body_phase() {
        assert_eq!(
            super::parse_pretended_hint("acct-1:2022"),
            vec![AccountSyncProgress {
                account_id: "acct-1".to_owned(),
                warming_bodies: true,
                bodies_done: 2022,
                ..AccountSyncProgress::default()
            }]
        );
    }

    #[test]
    fn an_unparseable_staged_hint_names_no_account() {
        for raw in ["", "acct-1", ":3/12", "acct-1:x/12", "acct-1:x"] {
            assert!(
                super::parse_pretended_hint(raw).is_empty(),
                "{raw:?} names no account"
            );
        }
    }
}
