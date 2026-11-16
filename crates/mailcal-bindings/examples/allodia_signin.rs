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
//! app over a temporary data directory, starts the sign-in as a client does
//! (`AllodiaIntent::SignIn`), prints the authorization URL the core holds, waits for the redirect
//! and hands it back; then **boots a second app** from what the first one stored, to prove the
//! entry comes back as a signed-in account and not as an account error. It signs out at the end,
//! so nothing is left behind but the temporary directory it names.

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use mailcal_bindings::{
    AccountCredentialStore, AccountPageIntent, AllodiaCard, AllodiaCardState, AllodiaIntent,
    BrowserSignInStage, CredentialStoreError, DeviceClass, DeviceInfo, Intent, LogLevel, Logger,
    MailcalApp, NoticeKind, Observer, Platform, SignInBrowserEnd, Surface,
    allodia_sign_in_available,
};

/// Waits until `done` answers, for as long as a person may take in the browser.
fn until<T>(mut done: impl FnMut() -> Option<T>) -> Result<T, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_mins(5);
    while Instant::now() < deadline {
        if let Some(found) = done() {
            return Ok(found);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("the core never got there".into())
}

/// Waits until the card says the sign-in or sign-out ended, and answers it.
fn settled(app: &MailcalApp) -> Result<AllodiaCard, Box<dyn std::error::Error>> {
    until(|| {
        let card = app.allodia_card();
        (card.browser.is_none() && !card.sign_out.running).then_some(card)
    })
}

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

    app.dispatch(Intent::Allodia {
        intent: AllodiaIntent::SignIn {
            create: false,
            redirect_uri: redirect_uri.clone(),
        },
    });
    let (out, url) = until(|| {
        let out = app.browser_sign_in()?;
        match &out.stage {
            BrowserSignInStage::InBrowser { url, .. } => Some((out.clone(), url.clone())),
            _ => None,
        }
    })?;
    println!("Open this, sign in, and come back:\n\n{url}\n");

    let callback_url = await_redirect(&listener, &redirect_uri)?;
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SignInBrowserEnded {
            account: out.account,
            attempt: out.attempt,
            end: SignInBrowserEnd::Redirected { callback_url },
        },
    });
    let card = settled(&app)?;
    let AllodiaCardState::SignedIn { account } = card.state else {
        return Err(format!("not signed in: {:?}", card.failure).into());
    };
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
    let notices = relaunched.notices().notices;
    println!(
        "  notices: {:?}",
        notices
            .iter()
            .map(|notice| &notice.kind)
            .collect::<Vec<_>>()
    );
    println!("  mail accounts: {}", relaunched.mailbox_list().rows.len());
    assert_eq!(relaunched.allodia_account().as_ref(), Some(&account));
    assert!(
        !notices
            .iter()
            .any(|notice| matches!(notice.kind, NoticeKind::AccountsSkipped { .. })),
        "every stored account came back"
    );

    relaunched.dispatch(Intent::Allodia {
        intent: AllodiaIntent::SignOut,
    });
    let card = settled(&relaunched)?;
    assert_eq!(
        card.state,
        AllodiaCardState::SignedOut,
        "{:?}",
        card.failure
    );
    println!(
        "  end the session at: {:?}",
        card.end_session.map(|end| end.url)
    );
    println!(
        "\nsigned out; the store now holds {} entry/entries",
        store.configs().len()
    );
    Ok(())
}
