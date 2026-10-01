//! Gated live check of signing in at Microsoft or Google for a chosen subset of what an account is
//! used for, through the FFI a client calls, with a person at the consent screen.
//!
//! The test opens the authorization URL in the desktop's browser, waits for the redirect on a
//! loopback port (the redirect the desktop clients register), completes the sign-in into a
//! throwaway app, and prints what the account stored and opened. With `MAILCAL_LIVE_ADD` set it
//! then signs the same account in again through `begin_account_consent`, adding those uses.
//!
//! Skips unless `MAILCAL_LIVE_CONSENT` names the provider. Needs the build's registrations
//! (`.env`, see `BUILDING.md`). By hand:
//! ```sh
//! MAILCAL_LIVE_CONSENT=microsoft MAILCAL_LIVE_CAPABILITIES=calendar MAILCAL_LIVE_ADD=contacts \
//!   cargo test -p mailcal-bindings --test live_provider_consent -- --nocapture
//! ```
//! Nothing is written outside a temp directory; the grant is left at the provider, where it can
//! be removed from the account's app permissions.

use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use mailcal_bindings::{
    AccountCapability, AccountCredentialStore, CredentialStoreError, DeviceClass, DeviceInfo,
    LogLevel, Logger, MailcalApp, Observer, Platform, Surface, begin_google_login,
    begin_microsoft_login,
};

struct PrintLogger;

impl Logger for PrintLogger {
    fn log(&self, level: LogLevel, target: String, message: String) {
        if !matches!(level, LogLevel::Debug | LogLevel::Trace) {
            eprintln!("[{level:?}] {target}: {message}");
        }
    }
}

struct SilentObserver(mpsc::Sender<()>);

impl Observer for SilentObserver {
    fn surface_changed(&self, _surface: Surface) {
        let _ = self.0.send(());
    }
}

#[derive(Clone, Default)]
struct RecordingStore(Arc<Mutex<Vec<String>>>);

impl AccountCredentialStore for RecordingStore {
    fn persist(
        &self,
        _account_id: String,
        config_toml: String,
    ) -> Result<(), CredentialStoreError> {
        self.0.lock().expect("store mutex").push(config_toml);
        Ok(())
    }

    fn delete(&self, _account_id: String) -> Result<(), CredentialStoreError> {
        Ok(())
    }
}

fn app(store: RecordingStore) -> Arc<MailcalApp> {
    let dir = std::env::temp_dir().join(format!("mailcal-live-consent-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a writable temp dir");
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(SilentObserver(tx)),
        Box::new(PrintLogger),
        LogLevel::Info,
        Vec::new(),
        dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        DeviceInfo {
            platform: Platform::Linux,
            os_version: "live".to_owned(),
            device_class: DeviceClass::LinuxDesktop,
            app_version: "0.0.0".to_owned(),
            locale: "en".to_owned(),
        },
        Box::new(store),
    )
    .expect("an account-less app boots")
}

fn capabilities(variable: &str) -> Option<Vec<AccountCapability>> {
    let list = std::env::var(variable).ok()?;
    Some(
        list.split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| match name {
                "mail" => AccountCapability::Mail,
                "calendar" => AccountCapability::Calendar,
                "contacts" => AccountCapability::Contacts,
                "colleagues" => AccountCapability::Colleagues,
                other => panic!("{variable}: unknown capability {other}"),
            })
            .collect(),
    )
}

/// Opens `url` in the desktop's browser and waits for the one redirect back to `listener`,
/// answering the browser with a page that says it can be closed.
fn browse(listener: &TcpListener, url: &str) -> String {
    eprintln!("\n==> opening the consent screen:\n{url}\n");
    let _ = std::process::Command::new("xdg-open").arg(url).status();
    let port = listener.local_addr().expect("bound").port();
    // A browser also asks the port for things that are not the redirect (`/favicon.ico` after
    // the previous tab), so wait for the request that carries the authorization response.
    loop {
        let (mut stream, _) = listener.accept().expect("the browser comes back");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("a read timeout");
        let mut line = String::new();
        let _ = BufReader::new(&stream).read_line(&mut line);
        let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
        let redirect = path.contains("state=");
        let body = if redirect {
            "Signed in. You can close this tab and return to the terminal."
        } else {
            ""
        };
        let status = if redirect { "200 OK" } else { "404 Not Found" };
        let _ = write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        if redirect {
            return format!("http://127.0.0.1:{port}{path}");
        }
    }
}

/// What the account stored, without its refresh token.
fn report(app: &MailcalApp, store: &RecordingStore, heading: &str) {
    // Give the first sync a moment to bind the calendar and contacts it opened.
    std::thread::sleep(Duration::from_secs(15));
    eprintln!("\n==> {heading}");
    if let Some(stored) = store.0.lock().expect("store mutex").last() {
        for line in stored.lines() {
            if !line.contains("refresh_token") && !line.contains("client_secret") {
                eprintln!("    {line}");
            }
        }
    }
    let connectivity = app.connectivity();
    eprintln!(
        "    calendar events: {}, contacts: {}, calendar re-consent: {:?}, mail re-consent: {:?}",
        app.calendar_list().events.len(),
        app.contact_list().rows.len(),
        connectivity.calendar_reauth_accounts,
        connectivity.mail_reauth_accounts,
    );
    if let Some(error) = app.calendar_connect_error() {
        eprintln!("    calendar connect error: {error}");
    }
}

#[test]
fn a_person_signs_in_for_what_they_chose() {
    let Ok(provider) = std::env::var("MAILCAL_LIVE_CONSENT") else {
        eprintln!("skipping the live consent test: MAILCAL_LIVE_CONSENT unset");
        return;
    };
    let chosen = capabilities("MAILCAL_LIVE_CAPABILITIES");
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let redirect_uri = format!(
        "http://127.0.0.1:{}/",
        listener.local_addr().unwrap().port()
    );
    let store = RecordingStore::default();
    let app = app(store.clone());

    let row = match provider.as_str() {
        "microsoft" => {
            let start = begin_microsoft_login(
                Some("common".to_owned()),
                redirect_uri.clone(),
                None,
                chosen,
            )
            .expect("this build carries a Microsoft registration");
            let callback = browse(&listener, &start.authorization_url);
            app.complete_microsoft_login(start.pending, callback)
        }
        "google" => {
            let start = begin_google_login(redirect_uri.clone(), None, chosen)
                .expect("this build carries a Google registration");
            let callback = browse(&listener, &start.authorization_url);
            app.complete_google_login(start.pending, callback)
        }
        other => panic!("MAILCAL_LIVE_CONSENT: unknown provider {other}"),
    }
    .unwrap_or_else(|error| panic!("the sign-in did not complete: {error}"));
    eprintln!("\n==> signed in as {} ({})", row.email, row.id);
    report(&app, &store, "after the first sign-in");

    let Some(adding) = capabilities("MAILCAL_LIVE_ADD") else {
        return;
    };
    let start = app
        .begin_account_consent(row.id.clone(), redirect_uri, adding)
        .expect("the account can sign in again");
    let callback = browse(&listener, &start.authorization_url);
    app.complete_account_consent(start.pending, callback)
        .unwrap_or_else(|error| panic!("signing in again did not complete: {error}"));
    report(&app, &store, "after signing in again");
}
