//! An account whose sign-in the server refused waits for the person, not for the network.
//!
//! Driven through the real constructor over a loopback JMAP server that answers every request
//! `401`, counting them, so the assertion is on what the server sees rather than on our own
//! bookkeeping.

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use crate::{
    Intent, LogLevel, MailcalApp, MailcalError,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

/// A JMAP server that refuses every credential, and how many requests it has answered.
fn refusing_server() -> (String, Arc<AtomicUsize>) {
    let requests = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&requests);
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let addr = listener.local_addr().expect("the bound address");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            seen.fetch_add(1, Ordering::SeqCst);
            let _ = stream.write_all(
                b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"mail\"\r\n\
                  Content-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    (format!("http://{addr}"), requests)
}

/// Waits up to five seconds for `done`.
fn wait_for(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_refused_sign_in_is_dialled_again_only_for_a_new_credential() {
    let (base_url, requests) = refusing_server();
    let config = format!(
        "[jmap]\nemail = \"refused@example.org\"\nbase_url = \"{base_url}\"\npassword = \"pw\"\n"
    );
    let id = mailcal_account::load_jmap_str(&config)
        .expect("a valid config")
        .account_id()
        .expect("a valid account id");
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config],
        crate::tests::temp_data_dir("signin-refused")
            .to_string_lossy()
            .into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots over an account that will not sign in");

    // The launch dials it once and is refused.
    wait_for("the launch's refusal", || {
        app.connectivity()
            .signin_expired_accounts
            .contains(&id.as_str().to_owned())
    });
    let after_launch = requests.load(Ordering::SeqCst);
    assert!(after_launch > 0, "the launch never reached the server");

    // Neither a refresh nor the network coming back is a new credential.
    app.dispatch(Intent::RefreshMail);
    app.dispatch(Intent::ReportNetworkReachable { reachable: true });
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        requests.load(Ordering::SeqCst),
        after_launch,
        "a refused sign-in was dialled again with the credential the server had just refused",
    );

    // A replaced credential is. This server refuses that too, which leaves the account as it was.
    let replaced = app.replace_account_secret(id.as_str().to_owned(), "new-pw".to_owned());
    assert!(
        matches!(replaced, Err(MailcalError::SignInRefused { .. })),
        "{replaced:?}"
    );
    assert!(
        requests.load(Ordering::SeqCst) > after_launch,
        "the replaced credential was never tried",
    );
}
