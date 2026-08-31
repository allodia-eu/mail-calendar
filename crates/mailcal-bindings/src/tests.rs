//! FFI-surface tests for the bindings: launch diagnostics, deferred boot outage handling,
//! device time-zone detection, and an end-to-end demo-app dispatch through the
//! fire-and-forget loop. Split out of `lib.rs` (via `#[path]`) to keep it under the
//! 500-line limit.

use std::{
    fs,
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use engine_api::{TimeZoneId, is_supported_zone};

use super::*;
use crate::boot::{DialFailure, FailedDial, record_dial_outcome};

/// A Rust [`Observer`] that forwards each signalled surface onto a channel, so a
/// synchronous test can wait for the fire-and-forget dispatch to complete. Shared with
/// `tests_calendar.rs` and `tests_showcase.rs`.
pub(crate) struct ChannelObserver {
    pub(crate) tx: mpsc::Sender<Surface>,
}

impl Observer for ChannelObserver {
    fn surface_changed(&self, surface: Surface) {
        let _ = self.tx.send(surface);
    }
}

/// A [`Logger`] that drops every record: the FFI tests don't assert on logs. Shared with
/// `tests_calendar.rs` and `tests_showcase.rs`.
pub(crate) struct NullLogger;

impl Logger for NullLogger {
    fn log(&self, _level: LogLevel, _target: String, _message: String) {}
}

/// A host credential store that records what it was asked to persist and what it was asked to
/// erase, so a test can ask whether the store the constructor was handed is the one an account's
/// credential actually reaches; in both directions.
#[derive(Default)]
pub(crate) struct RecordingCredentialStore {
    pub(crate) persisted: std::sync::Mutex<Vec<(String, String)>>,
    pub(crate) deleted: std::sync::Mutex<Vec<String>>,
}

impl AccountCredentialStore for RecordingCredentialStore {
    fn persist(
        &self,
        account_id: String,
        config_toml: String,
    ) -> Result<(), crate::CredentialStoreError> {
        self.persisted
            .lock()
            .expect("recorder mutex poisoned")
            .push((account_id, config_toml));
        Ok(())
    }

    fn delete(&self, account_id: String) -> Result<(), crate::CredentialStoreError> {
        self.deleted
            .lock()
            .expect("recorder mutex poisoned")
            .push(account_id);
        Ok(())
    }
}

/// Hands the *same* recorder to the constructor that the test keeps a handle on: the FFI takes
/// a `Box`, so the test cannot otherwise observe what it passed in.
pub(crate) struct RecordingStoreHandle(pub(crate) std::sync::Arc<RecordingCredentialStore>);

impl AccountCredentialStore for RecordingStoreHandle {
    fn persist(
        &self,
        account_id: String,
        config_toml: String,
    ) -> Result<(), crate::CredentialStoreError> {
        self.0.persist(account_id, config_toml)
    }

    fn delete(&self, account_id: String) -> Result<(), crate::CredentialStoreError> {
        self.0.delete(account_id)
    }
}

pub(crate) fn temp_data_dir(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "mailcal-bindings-{name}-{}-{nanos}",
        std::process::id(),
    ))
}

pub(super) fn wait_until(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if pred() {
            return true;
        }
        thread::sleep(Duration::from_millis(200));
    }
    pred()
}

#[test]
fn launch_diagnostics_keep_disconnected_accounts_and_split_from_caldav_failures() {
    // Four launch outcomes over the shared classifier: a disconnected account (mail connect
    // failed → kept as a placeholder so it still lists, badged unreachable), an account whose
    // credential the server *refused* (kept the same way, but prompted rather than badged), an
    // account that came up but whose CalDAV failed, and a clean connect. The account stand-in is a
    // `&str` (the classification is provider-agnostic).
    let mut accounts: Vec<&str> = Vec::new();
    let mut account_errors = Vec::new();
    let mut calendar_errors = Vec::new();
    let mut failed: Vec<FailedDial> = Vec::new();

    record_dial_outcome(
        "ann-account",
        "ann@host".to_owned(),
        Some(DialFailure::MailFailed {
            detail: "ann@host: connection refused".to_owned(),
            signin_rejected: false,
        }),
        &mut accounts,
        &mut account_errors,
        &mut calendar_errors,
        &mut failed,
    );
    record_dial_outcome(
        "dee-account",
        "dee@host".to_owned(),
        Some(DialFailure::MailFailed {
            detail: "dee@host: sign-in rejected: imap: IMAP authentication failed".to_owned(),
            signin_rejected: true,
        }),
        &mut accounts,
        &mut account_errors,
        &mut calendar_errors,
        &mut failed,
    );
    record_dial_outcome(
        "bob-account",
        "bob@host".to_owned(),
        Some(DialFailure::CalendarOnly(
            "bob@host: caldav unreachable".to_owned(),
        )),
        &mut accounts,
        &mut account_errors,
        &mut calendar_errors,
        &mut failed,
    );
    record_dial_outcome(
        "cy-account",
        "cy@host".to_owned(),
        None,
        &mut accounts,
        &mut account_errors,
        &mut calendar_errors,
        &mut failed,
    );

    // EVERY account is kept now; even the one whose mail connect failed. It lists as a
    // placeholder with an outage badge (the old behaviour dropped it entirely).
    assert_eq!(
        accounts,
        vec!["ann-account", "dee-account", "bob-account", "cy-account"]
    );

    // Both mail failures are queued for reconnect, carrying their labelled detail, and the verdict
    // that decides whether the user sees an outage badge or is asked to sign in again.
    let queued: Vec<(&str, &str, bool)> = failed
        .iter()
        .map(|failure| {
            (
                failure.id.as_str(),
                failure.detail.as_str(),
                failure.signin_rejected,
            )
        })
        .collect();
    assert_eq!(
        queued,
        vec![
            ("ann@host", "ann@host: connection refused", false),
            (
                "dee@host",
                "dee@host: sign-in rejected: imap: IMAP authentication failed",
                true,
            ),
        ],
    );

    // The disconnected mail account's error is in the account channel, NOT the calendar one.
    let account_channel = joined(&Mutex::new(account_errors)).expect("an account error");
    assert!(account_channel.contains("ann@host"));
    assert!(account_channel.contains("connection refused"));
    assert!(!account_channel.contains("caldav"));

    // The CalDAV-only failure is in the calendar channel, NOT the account one.
    let calendar_channel = joined(&Mutex::new(calendar_errors)).expect("a calendar error");
    assert!(calendar_channel.contains("bob@host"));
    assert!(calendar_channel.contains("caldav unreachable"));
    assert!(!calendar_channel.contains("connection refused"));

    // An all-clean launch surfaces nothing in either channel.
    assert!(joined(&Mutex::new(Vec::new())).is_none());
}

#[test]
fn deferred_boot_badges_an_unreachable_account_without_dropping_it() {
    let (tx, _rx) = mpsc::channel();
    let config = account_config_toml(AccountSetup {
        imap_host: "127.0.0.1:1".to_owned(),
        username: "outage@example.com".to_owned(),
        password: "not-used".to_owned(),
        smtp_host: None,
        caldav_base_url: None,
        imap_security: None,
        smtp_security: None,
    })
    .expect("valid account config");
    let data_dir = temp_data_dir("outage");

    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config],
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingCredentialStore::default()),
    )
    .expect("app boots with a provider-less placeholder");

    let account_id = "outage@example.com@127.0.0.1";
    let accounts = app.mailbox_list().accounts;
    assert_eq!(accounts.len(), 1, "the placeholder account is kept");
    assert_eq!(accounts[0].id, account_id);

    let badged = wait_until(Duration::from_secs(15), || {
        app.connectivity()
            .unreachable_accounts
            .iter()
            .any(|id| id == account_id)
    });
    assert!(badged, "the deferred background dial badges the account");

    let connectivity = app.connectivity();
    assert!(!connectivity.offline, "the device itself stays online");
    assert_eq!(connectivity.unreachable_accounts, vec![account_id]);
    let detail = app
        .connection_detail(account_id.to_owned())
        .expect("unreachable account has a technical detail");
    assert!(
        detail.contains("outage@example.com"),
        "the detail names the affected account: {detail}",
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn device_time_zone_is_a_resolvable_iana_zone() {
    // Detection is host-dependent (the CI machine's zone), so don't assert a city;
    // just that it returns a non-empty IANA id the engine can resolve (the detected
    // zone, or the Etc/UTC fallback when detection fails or the zone is unknown).
    let id = device_time_zone();
    assert!(!id.is_empty());
    assert!(is_supported_zone(
        &TimeZoneId::iana(&id).expect("non-empty zone id")
    ));
}

#[test]
fn demo_app_dispatches_through_the_ffi_loop_and_notifies() {
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    );

    // Initially empty, observer silent.
    assert!(app.mailbox_list().rows.is_empty());

    // Dispatch is fire-and-forget; the observer signals on completion. The refresh emits
    // background sync-progress signals before the mailbox list, so wait for the list (the demo's
    // data is ready once it fires).
    let wait_for_list = || {
        while let Ok(surface) = rx.recv_timeout(Duration::from_secs(5)) {
            if matches!(surface, Surface::MailboxList) {
                return true;
            }
        }
        false
    };
    app.dispatch(Intent::RefreshMail);
    assert!(
        wait_for_list(),
        "the mailbox-list surface fired within the timeout"
    );

    // This test exercises the flat projection (and then opens a specific message), so switch to
    // the flat view: the grouping now defaults to threaded.
    //
    // Waited on by *state*, not by the next signal. Publishes are not one per dispatch; the
    // body warm running behind the refresh above signals the list too: so a signal already
    // queued from that satisfies `wait_for_list` before the re-projection has landed, and the
    // snapshot read next is still the threaded one (three rows for these four messages, one
    // being a reply).
    app.dispatch(Intent::SetViewMode {
        mode: ViewMode::Flat,
    });
    let wait_for_flat = || {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            let rows = app.mailbox_list().rows;
            if !rows.is_empty()
                && rows
                    .iter()
                    .all(|row| matches!(row, SnapshotRow::Flat { .. }))
            {
                return true;
            }
            let _ = rx.recv_timeout(Duration::from_millis(50));
        }
        false
    };
    assert!(
        wait_for_flat(),
        "the flat re-projection landed within the timeout"
    );

    // The demo provider seeded four messages (one a reply), now flat rows in the snapshot; the
    // whole FFI loop (dispatch -> runtime -> app -> bridge -> observer) ran.
    let snapshot = app.mailbox_list();
    assert_eq!(snapshot.rows.len(), 4);
    assert!(snapshot.rows.iter().any(|row| match row {
        SnapshotRow::Flat { row } => row.subject.contains("Allodia"),
        SnapshotRow::Thread { .. } => false,
    }));

    // Opening a message fetches its body and signals the reading surface; the body
    // crosses the FFI sanitised: the demo's script + remote image are stripped, the
    // safe formatting kept.
    let (account, key) = match &snapshot.rows[0] {
        SnapshotRow::Flat { row } => (row.account.clone(), row.key.clone()),
        SnapshotRow::Thread { .. } => panic!("expected a flat row"),
    };
    app.dispatch(Intent::OpenMessage {
        account,
        key: key.clone(),
    });
    assert!(
        wait_until(Duration::from_secs(5), || matches!(
            rx.try_recv(),
            Ok(Surface::Reading)
        )),
        "the reading observer fired within the timeout",
    );
    let reading = app.reading_view();
    assert_eq!(reading.key, key);
    let html = reading.html.expect("an html body crossed the FFI");
    assert!(html.contains("demo"));
    // Script stripped; presentational HTML kept; the remote image is kept but flagged
    // (the WebView CSP gates the actual load until the user accepts the prompt).
    assert!(!html.contains("<script"));
    assert!(html.contains("tracker.example"));
    assert!(reading.has_remote_images);

    // The shared renderer wraps the fragment in a strict-CSP document; remote images
    // are blocked by default and load only once the user opts in.
    assert!(render_message_html(html.clone(), false).contains("img-src data:"));
    assert!(render_message_html(html, true).contains("http:"));
}

#[test]
fn settings_ffi_exposes_the_grouping_default_and_sync_depth_options() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    );
    // The grouping now defaults to threaded (a persisted preference; the demo has no prefs file).
    assert!(matches!(app.view_mode(), ViewMode::Threaded));
    // The per-account fetch-depth picker options + poll intervals cross the FFI record
    // conversion even with no real accounts: the settings screen builds its pickers from these.
    let settings = app.sync_settings();
    assert_eq!(settings.sync_depths, vec![3, 6, 9, 12, 24, 0]);
    assert_eq!(settings.poll_intervals, vec![15, 30, 60, 90, 120]);
}

#[test]
fn connection_info_crosses_the_ffi() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    );

    let infos = app.connection_info("demo".to_owned());
    assert_eq!(infos.len(), 1);
    assert!(infos[0].tls_version.is_none());
    assert!(infos[0].http_version.is_none());
    assert!(app.connection_info("missing".to_owned()).is_empty());
}

#[test]
fn external_link_policy_crosses_the_ffi() {
    // The shared launch policy is exposed to every client over the FFI: safe link schemes
    // open, custom app schemes / hostile schemes do not. (Policy lives in mailcal-app.)
    assert!(should_open_external_link("https://example.com".to_owned()));
    assert!(should_open_external_link("mailto:a@b.example".to_owned()));
    assert!(!should_open_external_link("myapp://home".to_owned()));
    assert!(!should_open_external_link("javascript:alert(1)".to_owned()));
}
