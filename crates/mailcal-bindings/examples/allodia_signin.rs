//! Drive a real Allodia sign-in **through the FFI**, from a terminal.
//!
//! Not a test: it needs a running service, a browser and a person. What it proves that
//! `allodia-license`'s own probe cannot is the path a client actually takes: the `pending` handle
//! surviving the browser round trip, the grant reaching the host's store, and the next launch
//! finding it there instead of reporting a corrupt account.
//!
//!     cargo run -p mailcal-bindings --features allodia-license --example allodia_signin
//!
//! It binds a loopback listener (what the Linux client and every desktop OAuth flow do), boots an
//! app over a temporary data directory, prints the authorization URL to open, waits for the
//! redirect, completes the sign-in; then **boots a second app** from what the first one stored, to
//! prove the entry comes back as a signed-in account and not as an account error. It signs out at
//! the end, so nothing is left behind but the temporary directory it names.

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    sync::{Arc, Mutex},
};

use mailcal_bindings::{
    AccountCredentialStore, CredentialStoreError, DeviceClass, DeviceInfo, LogLevel, Logger,
    MailcalApp, Observer, Platform, Surface, allodia_sign_in_available,
};

/// A host store, in memory. The real ones are a Keychain, a Credential Manager and an Android
/// Keystore; what matters for this probe is only that it is the same port, keyed the same way.
#[derive(Default)]
struct MemoryStore {
    entries: Mutex<BTreeMap<String, String>>,
}

impl MemoryStore {
    /// Every stored config, in id order; what a host hands the next launch.
    fn configs(&self) -> Vec<String> {
        self.entries
            .lock()
            .expect("store")
            .values()
            .cloned()
            .collect()
    }
}

impl AccountCredentialStore for MemoryStore {
    fn persist(&self, account_id: String, config_toml: String) -> Result<(), CredentialStoreError> {
        println!(
            "  store: persist {account_id} ({} bytes)",
            config_toml.len()
        );
        self.entries
            .lock()
            .expect("store")
            .insert(account_id, config_toml);
        Ok(())
    }

    fn delete(&self, account_id: String) -> Result<(), CredentialStoreError> {
        println!("  store: delete {account_id}");
        self.entries.lock().expect("store").remove(&account_id);
        Ok(())
    }
}

/// The store is shared with the probe (which reads back what was written) and handed to two apps,
/// so what the constructor takes is a handle to it rather than the store itself.
struct SharedStore(Arc<MemoryStore>);

impl AccountCredentialStore for SharedStore {
    fn persist(&self, account_id: String, config_toml: String) -> Result<(), CredentialStoreError> {
        self.0.persist(account_id, config_toml)
    }

    fn delete(&self, account_id: String) -> Result<(), CredentialStoreError> {
        self.0.delete(account_id)
    }
}

struct SilentObserver;
impl Observer for SilentObserver {
    fn surface_changed(&self, _surface: Surface) {}
}

struct StderrLogger;
impl Logger for StderrLogger {
    fn log(&self, level: LogLevel, target: String, message: String) {
        if matches!(level, LogLevel::Warn | LogLevel::Error) {
            eprintln!("  [{target}] {message}");
        }
    }
}

fn device() -> DeviceInfo {
    DeviceInfo {
        platform: Platform::Macos,
        os_version: "0.0".to_owned(),
        device_class: DeviceClass::MacLaptop,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        locale: "en-GB".to_owned(),
    }
}

/// Waits for one redirect and returns the full callback URL.
fn await_redirect(listener: &TcpListener, redirect_uri: &str) -> Result<String, String> {
    let (stream, _) = listener.accept().map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|error| error.to_string())?;
    let target = request_line
        .split_whitespace()
        .nth(1)
        .ok_or("no request target")?;
    let mut stream = stream;
    let _ = stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nSigned in. You can close this tab.\r\n",
    );
    Ok(format!("{}{}", redirect_uri.trim_end_matches('/'), target))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !allodia_sign_in_available() {
        eprintln!(
            "This build carries no Allodia client registration, so it offers no sign-in.\n\
             Set MAILCAL_ALLODIA_CLIENT_ID (and MAILCAL_ALLODIA_HOST for a local service) and \
             rebuild; see BUILDING.md."
        );
        return Ok(());
    }

    let data_dir =
        std::env::temp_dir().join(format!("allodia-signin-probe-{}", std::process::id()));
    std::fs::create_dir_all(&data_dir)?;
    let store = Arc::new(MemoryStore::default());
    println!("data dir: {}", data_dir.display());

    let app = MailcalApp::new_accounts(
        Box::new(SilentObserver),
        Box::new(StderrLogger),
        LogLevel::Info,
        Vec::new(),
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        device(),
        Box::new(SharedStore(Arc::clone(&store))),
    )?;

    // Port 0: the OS picks. The redirect the service sees is the one it is told, so this needs no
    // registration beyond the loopback allowance every OAuth server makes (RFC 8252 §7.3).
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
    let redirect_uri = format!("http://127.0.0.1:{}/", listener.local_addr()?.port());
    println!("redirect: {redirect_uri}\n");

    let start = app.begin_allodia_sign_in(redirect_uri.clone())?;
    println!(
        "Open this, sign in, and come back:\n\n{}\n",
        start.authorization_url
    );

    let callback_url = await_redirect(&listener, &redirect_uri)?;
    let account = app.complete_allodia_sign_in(start.pending, callback_url)?;
    println!("\nsigned in: {} ({:?})", account.email, account.name);
    assert_eq!(app.allodia_account().as_ref(), Some(&account));

    // The half no unit test can reach: what the host stored is what the next launch is handed, and
    // the next launch has to recognise it rather than fail to parse it as a mailbox.
    println!("\nre-booting from the stored configs…");
    let relaunched = MailcalApp::new_accounts(
        Box::new(SilentObserver),
        Box::new(StderrLogger),
        LogLevel::Info,
        store.configs(),
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        device(),
        Box::new(SharedStore(Arc::clone(&store))),
    )?;
    println!("  restored: {:?}", relaunched.allodia_account());
    println!("  account errors: {:?}", relaunched.account_connect_error());
    println!("  mail accounts: {}", relaunched.mailbox_list().rows.len());
    assert_eq!(relaunched.allodia_account().as_ref(), Some(&account));
    assert_eq!(relaunched.account_connect_error(), None);

    relaunched.sign_out_of_allodia()?;
    println!(
        "\nsigned out; the store now holds {} entry/entries",
        store.configs().len()
    );
    Ok(())
}
