//! The account-lifecycle log, asserted end to end against a capturing host [`Logger`].
//!
//! `credential_log`'s own unit tests pin what each line *says*, nothing there proves any of it
//! ever reaches a log. This does the other half: it drives the real public FFI surface; the
//! constructor, `add_account`, `remove_account`; through a [`Logger`] that keeps every record,
//! and asserts on what a support engineer would actually be reading.
//!
//! # Why this is an integration test and not a unit one
//!
//! [`crate::logging::install_logger`] swaps a **process-global** sink, and every `MailcalApp`
//! constructor calls it. The crate's unit tests build dozens of apps and cargo runs them in
//! parallel in one process, so a capture installed in `src/` would lose its records the moment any
//! other test constructed an app; intermittently, which is worse than not testing it. Cargo gives
//! each integration file its own process; this one builds every app it needs itself, so nothing
//! can take the sink away.
//!
//! For the same reason the whole thing is **one** `#[test]`: two cases here would race each other.

use std::{
    fs,
    sync::{Arc, Mutex, mpsc},
};

use mailcal_bindings::{
    AccountCredentialStore, CredentialStoreError, DeviceClass, DeviceInfo, LogLevel, Logger,
    MailcalApp, Observer, Platform, Surface,
};

/// A host logger that keeps every record the core emits.
struct Capture(Arc<Mutex<Vec<String>>>);

impl Logger for Capture {
    fn log(&self, _level: LogLevel, target: String, message: String) {
        self.0
            .lock()
            .expect("capture mutex poisoned")
            .push(format!("{target} {message}"));
    }
}

/// The observer the FFI requires; this test asserts on logs, not snapshots.
struct SilentObserver(mpsc::Sender<()>);

impl Observer for SilentObserver {
    fn surface_changed(&self, _surface: Surface) {
        let _ = self.0.send(());
    }
}

/// A store that accepts everything, so the *successful* paths are exercised.
struct AcceptingStore;

impl AccountCredentialStore for AcceptingStore {
    fn persist(
        &self,
        _account_id: String,
        _config_toml: String,
    ) -> Result<(), CredentialStoreError> {
        Ok(())
    }

    fn delete(&self, _account_id: String) -> Result<(), CredentialStoreError> {
        Ok(())
    }
}

/// A store that refuses everything: a locked Keychain, an Android Keystore key invalidated by a
/// biometric enrolment, a Credential Manager entry over its size cap.
struct RefusingStore;

impl AccountCredentialStore for RefusingStore {
    fn persist(
        &self,
        _account_id: String,
        _config_toml: String,
    ) -> Result<(), CredentialStoreError> {
        Err(CredentialStoreError::Store(
            "the keychain is locked".to_owned(),
        ))
    }

    fn delete(&self, _account_id: String) -> Result<(), CredentialStoreError> {
        Err(CredentialStoreError::Store(
            "the keychain is locked".to_owned(),
        ))
    }
}

/// An address that cannot occur by accident, so "did this leak?" is a substring search that
/// cannot collide with an unrelated line. Pointed at a port nothing listens on, so every connect
/// fails at once and the test needs no network.
const EMAIL: &str = "zelphina@carbuncle.test";
const UNREACHABLE: &str = "http://127.0.0.1:1";

fn jmap_config_toml() -> String {
    format!(
        "[jmap]\nemail = \"{EMAIL}\"\nbase_url = \"{UNREACHABLE}\"\npassword = \"quorrix-pw\"\n",
    )
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mailcal-credlog-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a writable temp dir");
    dir
}

fn app(
    lines: &Arc<Mutex<Vec<String>>>,
    store: Box<dyn AccountCredentialStore>,
    dir: &std::path::Path,
) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(SilentObserver(tx)),
        Box::new(Capture(Arc::clone(lines))),
        LogLevel::Info,
        Vec::new(),
        dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        // `analytics::test_device` is crate-private, and this file is deliberately outside the
        // crate (see the module docs), so the fixture is spelled out. Nothing here is sent
        // anywhere: a build with no relay endpoint dispatches nothing at all.
        DeviceInfo {
            platform: Platform::Macos,
            os_version: "15.0".to_owned(),
            device_class: DeviceClass::MacLaptop,
            app_version: "0.0.0".to_owned(),
            locale: "en".to_owned(),
        },
        store,
    )
    .expect("an account-less app boots")
}

#[test]
fn the_account_lifecycle_is_readable_from_the_log_and_carries_no_address() {
    let lines = Arc::new(Mutex::new(Vec::new()));
    // Every line the whole test produced, accumulated across the phases below. The phases each
    // assert on their *own* output, so each takes the buffer and empties it, and the privacy
    // sweep at the end has to see all of it, not just whatever the last phase left behind. It did
    // exactly that at first, which made the sweep pass while a leak sat in a phase it never
    // looked at: an assertion that examines nothing is the failure mode this test is *for*.
    let mut all: Vec<String> = Vec::new();

    // --- An add whose connect fails ------------------------------------------------------------
    // The case a support log most needs to explain, and the one that used to produce a log in
    // which nothing had happened: `add_account` had no log statements at all, so an IMAP/JMAP add
    // appeared only as raw `connect[…]` steps, and an add that *hung* appeared as nothing.
    let failed_dir = temp_dir("failed-add");
    let failed = app(&lines, Box::new(AcceptingStore), &failed_dir);
    assert!(
        failed.add_account(jmap_config_toml()).is_err(),
        "nothing is listening on 127.0.0.1:1",
    );

    let captured = drain(&lines, &mut all);
    assert_logged(&captured, "add-account: connecting a new jmap account");
    assert_logged(&captured, "add-account: the jmap connect failed");
    // Not "and nothing was stored". A sign-in whose very first refresh rotates the token before the
    // mail host refuses has already had that grant written by the sink, and losing it would be
    // worse than keeping it: so the line stops at what it can actually promise.
    assert_logged(&captured, "the account was not added");

    // --- A removal that erases -----------------------------------------------------------------
    // The property `delete` exists for, and the line that says it happened. `remove_account`
    // erases through the same port the write went through, so this is the log's evidence that the
    // account will not come back at the next launch.
    let account_id = format!("{EMAIL}@127.0.0.1:1");
    failed
        .remove_account(account_id.clone())
        .expect("the accepting store takes the erase");
    let captured = drain(&lines, &mut all);
    assert_logged(&captured, "erased this account's credential");
    assert_logged(&captured, "not come back at the next launch");

    // --- A removal the store refuses -----------------------------------------------------------
    // A zombie in the making: the account is gone from the app, its credential is not, and the
    // next launch brings it back with nothing to explain it. That has to be in the log, because
    // it is the only place it will ever be visible.
    let refusing_dir = temp_dir("refusing");
    let refusing = app(&lines, Box::new(RefusingStore), &refusing_dir);
    assert!(
        refusing.remove_account(account_id.clone()).is_err(),
        "a refused erase reaches the caller",
    );
    let captured = drain(&lines, &mut all);
    assert_logged(&captured, "REFUSED to erase this account's credential");
    assert_logged(&captured, "the keychain is locked");
    assert_logged(&captured, "will come back as an account at the next launch");

    // --- The privacy half ----------------------------------------------------------------------
    // Over every line this test produced, not only the credential ones: `docs/logging.md` forbids
    // an address, a username or a host in a file the app invites the user to attach to a support
    // request, and the account id is all three. Asserted here rather than trusted, because writing
    // `{id}` instead of the handle is a one-word edit at every call site.
    drain(&lines, &mut all);
    assert!(
        all.len() > 5,
        "the sweep below examined almost nothing, so it proves nothing: {all:?}",
    );
    for line in &all {
        for forbidden in [EMAIL, "zelphina", "carbuncle", account_id.as_str()] {
            assert!(
                !line.contains(forbidden),
                "{forbidden:?} reached the diagnostic log: {line}",
            );
        }
    }

    drop(failed);
    drop(refusing);
    let _ = fs::remove_dir_all(failed_dir);
    let _ = fs::remove_dir_all(refusing_dir);
}

/// Takes everything captured so far, empties the buffer for the next phase, and keeps a copy in
/// `all` for the privacy sweep at the end. Both halves matter: a phase must assert on its own
/// output, and the sweep must still see every line the test ever produced.
fn drain(lines: &Arc<Mutex<Vec<String>>>, all: &mut Vec<String>) -> Vec<String> {
    let mut buffer = lines.lock().expect("capture mutex poisoned");
    let taken = std::mem::take(&mut *buffer);
    all.extend(taken.iter().cloned());
    taken
}

/// Asserts some captured line contains `needle`, printing the buffer when it does not: a bare
/// `any(...)` on a missing log line is otherwise undebuggable.
fn assert_logged(lines: &[String], needle: &str) {
    assert!(
        lines.iter().any(|line| line.contains(needle)),
        "no log line contains {needle:?}; captured:\n{}",
        lines.join("\n"),
    );
}
