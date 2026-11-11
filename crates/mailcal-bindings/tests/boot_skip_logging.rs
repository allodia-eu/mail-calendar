//! A stored account the core cannot read at boot is named in the diagnostic log, without any of
//! the stored text.
//!
//! An integration test for the reason `credential_logging.rs` gives: the logger is process-global,
//! so a capture installed beside the crate's parallel unit tests would lose records. One `#[test]`
//! for the same reason.

use std::{
    fs,
    sync::{Arc, Mutex, mpsc},
};

use mailcal_bindings::{
    AccountCredentialStore, CredentialStoreError, DeviceClass, DeviceInfo, LogLevel, Logger,
    MailcalApp, Observer, Platform, Surface,
};

struct Capture(Arc<Mutex<Vec<String>>>);

impl Logger for Capture {
    fn log(&self, _level: LogLevel, target: String, message: String) {
        self.0
            .lock()
            .expect("capture mutex poisoned")
            .push(format!("{target} {message}"));
    }
}

struct SilentObserver(mpsc::Sender<()>);

impl Observer for SilentObserver {
    fn surface_changed(&self, _surface: Surface) {
        let _ = self.0.send(());
    }
}

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

/// Values that cannot occur by accident, so a leak is a substring search.
const EMAIL: &str = "zelphina@carbuncle.test";
const TOKEN: &str = "quorrix-refresh-token";

#[test]
fn a_stored_account_left_out_at_boot_is_logged_without_its_contents() {
    let configs = vec![
        // What a keyring that keeps only the first line of a secret hands back: the account's
        // uses, and none of its sign-in.
        "capabilities = [\"mail\", \"calendar\", \"contacts\"]".to_owned(),
        // A config whose parse error quotes a line holding a secret.
        format!("[google]\nemail = \"{EMAIL}\"\nrefresh_token = \"{TOKEN}\" broken\n"),
    ];
    let dir = std::env::temp_dir().join(format!("mailcal-bootskip-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a writable temp dir");

    let lines = Arc::new(Mutex::new(Vec::new()));
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(SilentObserver(tx)),
        Box::new(Capture(Arc::clone(&lines))),
        LogLevel::Info,
        configs,
        dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        DeviceInfo {
            platform: Platform::Linux,
            os_version: "0".to_owned(),
            device_class: DeviceClass::Pc,
            app_version: "0.0.0".to_owned(),
            locale: "en".to_owned(),
        },
        Box::new(AcceptingStore),
    )
    .expect("an app whose stored accounts are unreadable still boots");

    let captured = lines.lock().expect("capture mutex poisoned").clone();
    for position in ["stored account 1 of 2", "stored account 2 of 2"] {
        assert!(
            captured
                .iter()
                .any(|line| line.contains(position) && line.contains("was left out")),
            "no line says {position:?} was left out; captured:\n{}",
            captured.join("\n"),
        );
    }
    for line in &captured {
        for forbidden in [EMAIL, "zelphina", "carbuncle", TOKEN, "quorrix"] {
            assert!(
                !line.contains(forbidden),
                "{forbidden:?} reached the diagnostic log: {line}",
            );
        }
    }

    drop(app);
    let _ = fs::remove_dir_all(dir);
}
