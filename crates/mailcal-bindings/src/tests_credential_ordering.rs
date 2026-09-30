//! One property, asserted separately on **each of the three paths that open an account**: a
//! refresh-token rotation that happens while the account is being connected reaches the host's
//! credential store.
//!
//! The paths are the cold **background worker**, the **foreground app**, and **adding an account**.
//! They are tested separately on purpose. They now share one dial and one registry, so in principle
//! one test would do, but "they share it" is exactly what was believed before, and the divergence
//! that killed a real account lived in the one path nobody had a test for. Three tests cost
//! seconds; being wrong about which paths are really the same cost a grant twice.
//!
//! Each drives the real FFI constructor over a **live loopback token endpoint that rotates**, with
//! a mail host that refuses. That combination is deliberate: `connect_one` mints the access token
//! *before* it dials the server, so the refresh (and its rotation) happens whether or not the
//! mailbox is reachable, and the dial then fails fast instead of hanging on a network we do not
//! have.
//!
//! Every cheaper way of asking this question cannot fail. Firing the sink *after* the constructor
//! returns passes either way, because by then the account is registered however late, and that is
//! precisely the test that existed while the bug shipped. The window is *inside* the call, so the
//! test has to make a rotation happen there.

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};

use crate::{
    LogLevel, MailcalApp,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

/// An OAuth JMAP account whose token endpoint answers and rotates, and whose **mail host refuses**.
///
/// `who` gives each test its **own** account, and that is load-bearing rather than tidy: token
/// state is shared per account across a process now, so two tests using one address would share it
/// , and the second would find a cached access token and never refresh at all. That is correct
/// behaviour and a broken test, which is exactly the shape `two_cores_over_one_account…` below
/// asserts on purpose.
fn rotating_account(who: &str, endpoint: String) -> (String, engine_api::AccountId) {
    let config = mailcal_account::JmapAccountConfig {
        email: format!("{who}@example.com"),
        // Nothing listens here, so the mail connect fails right after the refresh succeeds.
        base_url: "http://127.0.0.1:1".to_owned(),
        password: None,
        token: None,
        oauth: Some(mailcal_account::OAuthGrant {
            client_id: "client-abc".to_owned(),
            client_secret: None,
            refresh_token: mailcal_account::Secret::new("original-refresh".to_owned()),
            authorize_endpoint: "http://127.0.0.1:1/authorize".to_owned(),
            token_endpoint: endpoint,
            redirect_uri: "eu.allodia.mailcal://jmap-oauth".to_owned(),
            scopes: vec!["offline_access".to_owned()],
            resource: None,
            issuer: None,
        }),
    };
    let id = config.account_id().expect("a valid account id");
    (config.to_toml().expect("serializable config"), id)
}

/// Asserts the recorder holds `expected_id`'s config carrying the **rotated** refresh token.
fn assert_rotation_reached_the_store(
    recorder: &RecordingCredentialStore,
    expected_id: &engine_api::AccountId,
    context: &str,
) {
    let written = recorder.persisted.lock().expect("recorder mutex poisoned");
    let (persisted_id, toml) = written.first().unwrap_or_else(|| {
        panic!(
            "{context}: the rotation never reached the store. That is the shape of the bug: the \
             account was connected before it was registered, so the token sink had no entry to \
             update; the next launch then presents the superseded token and a ratcheting server \
             revokes the grant",
        )
    });
    assert_eq!(persisted_id, expected_id.as_str());
    let parsed = mailcal_account::load_jmap_str(toml).expect("valid config TOML");
    assert_eq!(
        parsed.oauth.expect("an oauth grant").refresh_token.expose(),
        "rotated-1",
        "{context}: the persisted config carries the token the rotation replaced",
    );
}

/// **The cold background worker.** It connects every account synchronously and is dropped at the
/// end of its pass, so there is no later moment in which to save a rotation. This is the path that
/// killed a real Fastmail account twice, ~2 days apart.
#[test]
fn a_rotation_during_a_cold_background_worker_connect_reaches_the_store() {
    let (endpoint, refreshes) = rotating_token_endpoint("original-refresh");
    let (config_toml, account_id) = rotating_account("cold-worker", endpoint);
    let data_dir = temp_data_dir("worker-rotation");
    let recorder = Arc::new(RecordingCredentialStore::default());
    let (tx, _rx) = mpsc::channel();

    let app = MailcalApp::new_background_worker(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config_toml],
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(&recorder))),
    )
    .expect("a headless worker boots even though the account is unreachable");

    assert_eq!(
        refreshes.load(Ordering::SeqCst),
        1,
        "the connect never refreshed, so this test is not exercising a rotation at all",
    );
    assert_rotation_reached_the_store(&recorder, &account_id, "cold background worker");

    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

/// **The foreground app.** It returns from the constructor with placeholders and dials in the
/// background, so the rotation lands *after* the host is already painting, which is why this waits
/// for it rather than asserting immediately.
///
/// This path was correct before the fix and is included anyway: it now shares its dial and its
/// registration with the worker, and the whole lesson here is that "these two paths are the
/// same" is a claim worth a test rather than a comment.
#[test]
fn a_rotation_during_the_foreground_background_dial_reaches_the_store() {
    let (endpoint, refreshes) = rotating_token_endpoint("original-refresh");
    let (config_toml, account_id) = rotating_account("foreground", endpoint);
    let data_dir = temp_data_dir("foreground-rotation");
    let recorder = Arc::new(RecordingCredentialStore::default());
    let (tx, _rx) = mpsc::channel();

    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config_toml],
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(&recorder))),
    )
    .expect("the app boots with a provider-less placeholder");

    // The dial is spawned in the constructor's last statement, so poll rather than sleep a fixed
    // amount: on a loopback endpoint with a refused mail host this lands in milliseconds.
    let arrived = crate::tests::wait_until(std::time::Duration::from_secs(15), || {
        !recorder
            .persisted
            .lock()
            .expect("recorder mutex poisoned")
            .is_empty()
    });
    assert!(
        arrived,
        "the background dial never persisted a rotation ({} refresh(es) reached the endpoint)",
        refreshes.load(Ordering::SeqCst),
    );
    assert_rotation_reached_the_store(&recorder, &account_id, "foreground background dial");

    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

/// **Adding an account.** The dial fails here (no mail host), so the add fails and rolls back; but
/// the rotation happened first, and a grant the server has already advanced must not be lost just
/// because the mailbox was unreachable.
///
/// A *successful* add cannot be tested offline: it needs a JMAP session server as well as a token
/// endpoint. What this pins is the ordering, which is the half that broke.
#[test]
fn a_rotation_during_add_account_reaches_the_store_even_when_the_dial_fails() {
    let (endpoint, refreshes) = rotating_token_endpoint("original-refresh");
    let (config_toml, account_id) = rotating_account("added", endpoint);
    let data_dir = temp_data_dir("add-account-rotation");
    let recorder = Arc::new(RecordingCredentialStore::default());
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(&recorder))),
    )
    .expect("an account-less app boots");

    let added = app.add_account(config_toml);

    assert!(added.is_err(), "nothing is listening on the mail host");
    assert_eq!(
        refreshes.load(Ordering::SeqCst),
        1,
        "the add never refreshed, so this test is not exercising a rotation at all",
    );
    assert_rotation_reached_the_store(&recorder, &account_id, "add_account");
    assert!(
        !app.registry.contains(account_id.as_str()),
        "a failed add must leave no registry entry behind",
    );

    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

/// A token endpoint that answers one refresh and **rotates** the refresh token, like Fastmail.
///
/// Deliberately a real socket rather than an injected fake: the property under test is what the
/// boot path does with a rotation the *provider stack* produced, and a hand-driven sink call skips
/// every step where the ordering could go wrong. Returns the URL and the count of refreshes seen,
/// so a test can tell "the rotation was dropped" from "no refresh ever happened", which look
/// identical at the store.
fn rotating_token_endpoint(initial: &str) -> (String, Arc<AtomicUsize>) {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&refreshes);
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let addr = listener.local_addr().expect("the bound address");
    let initial = initial.to_owned();
    std::thread::spawn(move || {
        let mut generation = 0usize;
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let request = read_request(&mut stream);
            // Only count a request that actually presents the live refresh token, so a malformed
            // one cannot make the assertion above pass for the wrong reason.
            let body = if request.contains(&format!("refresh_token={initial}")) {
                seen.fetch_add(1, Ordering::SeqCst);
                generation += 1;
                format!(
                    r#"{{"token_type":"Bearer","expires_in":3600,"access_token":"AT-{generation}","refresh_token":"rotated-{generation}"}}"#
                )
            } else {
                r#"{"error":"invalid_grant"}"#.to_owned()
            };
            let status = if body.contains("\"error\"") {
                "400 Bad Request"
            } else {
                "200 OK"
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len(),
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{addr}/token"), refreshes)
}

/// Reads a whole small HTTP request, honouring `Content-Length`: a single `read` can stop on the
/// headers and leave the form body behind, which would make the mock above answer `invalid_grant`
/// to a perfectly good refresh and turn this into a flaky test.
fn read_request(stream: &mut TcpStream) -> String {
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    while let Ok(read) = stream.read(&mut buf) {
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&buf[..read]);
        let text = String::from_utf8_lossy(&raw).into_owned();
        let Some((headers, body)) = text.split_once("\r\n\r\n") else {
            continue;
        };
        let expected = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())?
            })
            .unwrap_or(0);
        if body.len() >= expected {
            break;
        }
    }
    String::from_utf8_lossy(&raw).into_owned()
}

/// **Two cores over one account refresh once.** The device finding, reduced to a test.
///
/// A host can construct the core twice in one process; on Android a one-time `MailSyncWorker` and
/// the periodic one overlapped, and `MailcalApplication.liveCore` is a `WeakReference`, so a worker
/// can miss a warm core and build a cold one beside it. Measured on a real device, two cold cores 6
/// ms apart produced this:
///
/// ```text
/// 10:58:26.473  oauth: jmap [acct:05f4]: refreshed in 307ms; ... the server ROTATED the refresh token
/// 10:58:26.641  oauth: jmap [acct:05f4]: refreshed in 302ms; ... the server ROTATED the refresh token
/// ```
///
/// Two rotations of one grant from two independent refreshers, each having read the same stored
/// token: so the second presented one the first had already superseded. On a ratcheting server
/// that revokes the grant, and the account is dead at the next launch having worked perfectly all
/// session.
///
/// The endpoint here **counts** refreshes, so the assertion is on the thing the server sees rather
/// than on our own bookkeeping. One is correct; two is a replay.
#[test]
fn two_cores_over_one_account_refresh_once() {
    let (endpoint, refreshes) = rotating_token_endpoint("original-refresh");
    let (config_toml, account_id) = rotating_account("two-cores", endpoint);
    let recorder = Arc::new(RecordingCredentialStore::default());

    // Two cores, built concurrently, exactly as two overlapping workers do it. Separate data dirs
    // so this is a test of the *credential* rather than of SQLite: the token state is keyed by
    // account across the whole process, which is the point; it does not care whose store is whose.
    let cores: Vec<_> = ["two-cores-a", "two-cores-b"]
        .into_iter()
        .map(|name| {
            let config_toml = config_toml.clone();
            let recorder = Arc::clone(&recorder);
            let data_dir = temp_data_dir(name);
            std::thread::spawn(move || {
                let (tx, _rx) = mpsc::channel();
                let app = MailcalApp::new_background_worker(
                    Box::new(ChannelObserver { tx }),
                    Box::new(NullLogger),
                    LogLevel::Info,
                    vec![config_toml],
                    data_dir.to_string_lossy().into_owned(),
                    "Etc/UTC".to_owned(),
                    crate::analytics::test_device(),
                    Box::new(RecordingStoreHandle(recorder)),
                )
                .expect("a headless worker boots even though the account is unreachable");
                (app, data_dir)
            })
        })
        .collect();
    let built: Vec<_> = cores
        .into_iter()
        .map(|handle| handle.join().expect("a worker thread panicked"))
        .collect();

    assert_eq!(
        refreshes.load(Ordering::SeqCst),
        1,
        "both cores refreshed the same credential: the second presented a refresh token the first \
         had already superseded, which is the replay a ratcheting server revokes the grant over",
    );
    assert_rotation_reached_the_store(&recorder, &account_id, "two concurrent cores");

    for (app, data_dir) in built {
        drop(app);
        let _ = fs::remove_dir_all(data_dir);
    }
}
