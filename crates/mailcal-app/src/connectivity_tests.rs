//! Tests for connectivity handling: the offline short-circuit that stops the app hammering a
//! dead network, the self-heal refresh when the device returns online, and the per-account
//! "can't reach its server" badges (distinct from a device-wide offline state). The re-consent and
//! expired-sign-in prompts live in `connectivity_prompt_tests.rs`; the shared fixtures
//! (FakeProvider, observer, helpers) in `tests_fakes.rs`.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, EmailAddress};
use fakes::{FakeProvider, account, app, flat_subjects};

use super::{Intent, Surface};
use crate::Account;

// The shared fixtures are also included by `tests.rs`; each test file compiles them into its
// own module tree, which is intentional (they share no state); silence the duplicate-load lint.
#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn offline_refresh_renders_cache_without_syncing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);

    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    assert!(app.connectivity().offline, "the device is offline");
    surfaces.lock().unwrap().clear();

    // A refresh while offline must not touch the network (no sync begins → no download bar),
    // but it MUST re-signal the mailbox list so the primed cache renders: a cold offline launch
    // whose boot-prime signal fired before the host wired its observer would otherwise stay blank.
    app.dispatch(Intent::RefreshMail).await;
    let signals = surfaces.lock().unwrap();
    assert!(
        signals.contains(&Surface::MailboxList),
        "offline refresh re-signals the cached mailbox so it renders",
    );
    assert!(
        !signals.contains(&Surface::SyncProgress),
        "no network sync begins while offline (no download-bar storm)",
    );
}

#[tokio::test]
async fn coming_back_online_triggers_a_self_heal_refresh() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);

    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    surfaces.lock().unwrap().clear();

    app.dispatch(Intent::ReportNetworkReachable(true)).await;

    let signalled = surfaces.lock().unwrap().clone();
    assert!(
        signalled.contains(&Surface::Connectivity),
        "the offline banner clears",
    );
    assert!(
        signalled.contains(&Surface::MailboxList),
        "returning online refreshes mail so it catches up without a restart",
    );
    assert!(!app.connectivity().offline, "no longer offline");
}

#[tokio::test]
async fn a_repeated_reachability_report_changes_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);

    // The app defaults to online, so re-reporting online is a no-op (hosts may re-report the
    // same value on every OS callback): no spurious refresh, no signal.
    app.dispatch(Intent::ReportNetworkReachable(true)).await;
    assert!(
        surfaces.lock().unwrap().is_empty(),
        "an unchanged reachability report signals nothing",
    );
}

#[tokio::test]
async fn an_account_that_cannot_reach_its_server_is_badged_unreachable() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account("a", FakeProvider::new()),     // healthy
            account("b", FakeProvider::failing()), // server unreachable
        ],
        &surfaces,
    );

    app.dispatch(Intent::RefreshMail).await;

    let snapshot = app.connectivity();
    assert!(!snapshot.offline, "the device itself is online");
    assert_eq!(
        snapshot.unreachable_accounts,
        vec!["b".to_string()],
        "only the failing account is badged, not the healthy one",
    );
    assert!(
        surfaces.lock().unwrap().contains(&Surface::Connectivity),
        "becoming unreachable signals the connectivity surface",
    );
}

#[tokio::test]
async fn the_outage_badge_sets_clears_and_signals_only_on_change() {
    // Drives the badge state machine directly (a real recovery relies on the engine releasing
    // its scope lease over time, which an immediate second sync can't observe): a becoming
    // unreachable signals, recovering clears + signals, and an unchanged report is silent.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("b", FakeProvider::new())], &surfaces);
    let id = AccountId::try_from("b").unwrap();

    app.set_account_reachable(&id, false);
    assert_eq!(
        app.connectivity().unreachable_accounts,
        vec!["b".to_string()]
    );
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));

    surfaces.lock().unwrap().clear();
    app.set_account_reachable(&id, true);
    assert!(
        app.connectivity().unreachable_accounts.is_empty(),
        "recovering clears the badge",
    );
    assert!(
        surfaces.lock().unwrap().contains(&Surface::Connectivity),
        "clearing the badge signals the surface",
    );

    surfaces.lock().unwrap().clear();
    app.set_account_reachable(&id, true);
    assert!(
        surfaces.lock().unwrap().is_empty(),
        "an unchanged reachability report signals nothing",
    );
}

#[tokio::test]
async fn a_boot_outage_carries_a_detail_a_reachable_resync_does_not_clobber() {
    // A boot connect failure seeds the outage with a rich technical detail (the connect error);
    // a later routine sync that still can't reach must NOT wipe that detail (it carries none).
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("b", FakeProvider::new())], &surfaces);
    let id = AccountId::try_from("b").unwrap();

    app.note_account_unreachable(&id, Some("imap: connection refused".to_owned()));
    assert_eq!(
        app.connectivity().unreachable_accounts,
        vec!["b".to_string()]
    );
    assert_eq!(
        app.connection_detail(&id).as_deref(),
        Some("imap: connection refused"),
    );
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));

    // A detail-less sync failure keeps the account unreachable and preserves the boot detail.
    surfaces.lock().unwrap().clear();
    app.set_account_reachable(&id, false);
    assert_eq!(
        app.connection_detail(&id).as_deref(),
        Some("imap: connection refused"),
        "a detail-less resync must not clobber the boot detail",
    );
    assert!(
        surfaces.lock().unwrap().is_empty(),
        "an unchanged outage (already unreachable) signals nothing",
    );

    // Recovering clears both the badge and the detail.
    app.set_account_reachable(&id, true);
    assert!(app.connectivity().unreachable_accounts.is_empty());
    assert!(
        app.connection_detail(&id).is_none(),
        "recovery clears the detail"
    );
}

#[tokio::test]
async fn a_disconnected_account_still_lists_and_keeps_its_outage_badge() {
    // The core of the per-account-outage contract: an account whose providers never connected (a
    // boot-time provider outage) is kept as a placeholder with EMPTY providers; it must still
    // appear in the switcher (not vanish), and a refresh over it must be a harmless no-op that
    // leaves its seeded outage badge + detail intact (an empty-provider sync is indeterminate, so
    // it never reads as "recovered"). This is the state the Windows hosts-file scenario produced.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let healthy = account("a", FakeProvider::new());
    let disconnected = Account {
        id: AccountId::try_from("b").unwrap(),
        providers: Vec::new(),
        calendar_providers: Vec::new(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new("b@example.com"),
    };
    let app = app(vec![healthy, disconnected], &surfaces);
    let id = AccountId::try_from("b").unwrap();

    // Boot seeds its outage badge with the connect error, exactly as the bindings do.
    app.note_account_unreachable(
        &id,
        Some("b@example.com: imap: connection refused".to_owned()),
    );

    // A refresh runs (the healthy account syncs); the disconnected account must NOT be dropped.
    app.dispatch(Intent::RefreshMail).await;
    let accounts = app.mailbox_list().accounts;
    assert!(
        accounts.iter().any(|row| row.id == "b"),
        "a disconnected account must still list in the switcher, not vanish",
    );
    assert!(
        accounts.iter().any(|row| row.id == "a"),
        "the healthy account lists alongside it",
    );

    // Its outage badge + detail survive the refresh: an empty-provider sync reports no
    // reachability signal, so the seeded badge stays put (never falsely cleared).
    assert!(
        app.connectivity()
            .unreachable_accounts
            .contains(&"b".to_string()),
        "the disconnected account keeps its outage badge across a refresh",
    );
    assert_eq!(
        app.connection_detail(&id).as_deref(),
        Some("b@example.com: imap: connection refused"),
        "and its technical detail (for the 'details' link) is preserved",
    );
}

#[tokio::test]
async fn going_offline_suppresses_per_account_warnings() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account("a", FakeProvider::new()),
            account("b", FakeProvider::failing()),
        ],
        &surfaces,
    );

    app.dispatch(Intent::RefreshMail).await;
    assert_eq!(
        app.connectivity().unreachable_accounts,
        vec!["b".to_string()]
    );

    // Once the whole device is offline, the single global banner stands in for the per-account
    // warnings (the fault is the device, not any one account).
    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    let snapshot = app.connectivity();
    assert!(snapshot.offline);
    assert!(
        snapshot.unreachable_accounts.is_empty(),
        "per-account badges are suppressed while the device is offline",
    );
}

#[tokio::test]
async fn an_offline_refresh_keeps_the_cached_mailbox_visible() {
    // The core guarantee behind instant offline launch (and the fix for an Android boot in
    // airplane mode showing an empty list): once mail is synced, going offline and refreshing must
    // NOT wipe the list: the offline refresh short-circuits, it doesn't rebuild to empty.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);

    // Sync while online: the mailbox now holds the account's cached mail.
    app.dispatch(Intent::RefreshMail).await;
    let cached = flat_subjects(&app.mailbox_list());
    assert!(
        cached.contains(&"Quarterly report".to_string()),
        "the online sync populated the mailbox",
    );

    // Go offline and refresh: the cached mail must remain visible.
    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    app.dispatch(Intent::RefreshMail).await;
    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        cached,
        "an offline refresh keeps the cached mailbox visible, never wiping it",
    );
}
