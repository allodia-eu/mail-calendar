//! The other devices' card and an account's sharing control through the app: drawn only while
//! someone is signed in, the control only for an account the other devices can hold, a sharing
//! change run as a change on the page that answers the questions about it, and a pass run by the
//! core when the bookkeeping arrives. No pass here asks the service: each answers what the test
//! put in `canned`.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    AccountPageIntent, AllodiaAccountSyncMode, AllodiaDevices, Intent, LogLevel, MailcalApp,
    allodia::StoredAccount,
    allodia_sync::{AllodiaAccountChange, AllodiaSyncReport},
    sync_state::{NoSyncState, StoredSyncState},
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const MAIL: &str = r#"
[imap]
addr = "localhost:1"
server_name = "localhost"
username = "alice@example.org"
password = "pw"
"#;

const ID: &str = "alice@example.org@localhost";

fn app(name: &str) -> Arc<MailcalApp> {
    app_with(name, MAIL)
}

fn app_with(name: &str, config: &str) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config.to_owned()],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots")
}

fn sign_in(app: &MailcalApp) {
    *app.allodia.lock().unwrap() = Some(StoredAccount {
        id: Some("person".to_owned()),
        email: "person@test.local".to_owned(),
        name: None,
        refresh_token: "RT".to_owned(),
        granted_scopes: None,
        end_session_endpoint: None,
    });
}

fn asked_about_alice() -> AllodiaSyncReport {
    AllodiaSyncReport {
        changed_elsewhere: vec![AllodiaAccountChange {
            account_id: ID.to_owned(),
            email: "alice@example.org".to_owned(),
            also_changed_here: false,
        }],
        ..AllodiaSyncReport::default()
    }
}

/// Signed in, with a store whose note says alice was sent, and every pass answering `canned`.
fn signed_in(name: &str, canned: AllodiaSyncReport) -> Arc<MailcalApp> {
    let app = app(name);
    sign_in(&app);
    *app.allodia_sync.canned.lock().unwrap() = Some(Ok(canned));
    app.use_allodia_sync_state_store(Box::new(NoSyncState))
        .expect("the store is read");
    let sent = StoredSyncState {
        id: "record".to_owned(),
        version: 1,
        fingerprint: "fingerprint".to_owned(),
    };
    app.allodia_sync
        .bookkeeping()
        .expect("installed")
        .set(ID, sent)
        .expect("noted");
    app
}

/// The card once no pass and no change runs any more.
fn settled(app: &MailcalApp) -> Option<AllodiaDevices> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let card = app.allodia_devices();
        let checking = card.as_ref().is_some_and(|card| card.checking);
        if !checking && !app.account_work.of(ID).0.busy {
            return card;
        }
        assert!(Instant::now() < deadline, "the pass never ended");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn nothing_is_drawn_while_nobody_is_signed_in() {
    let app = app("devices-signed-out");
    app.use_allodia_sync_state_store(Box::new(NoSyncState))
        .expect("the store is read");
    assert_eq!(app.allodia_devices(), None);
    let page = app.account_page(ID.to_owned()).expect("listed");
    assert_eq!(page.sharing, None);
}

#[test]
fn installing_the_store_runs_a_pass_and_the_card_draws_what_it_found() {
    let app = signed_in("devices-pass", asked_about_alice());
    let card = settled(&app).expect("the pass found something");
    assert_eq!(card.questions.len(), 1);
    assert_eq!(card.questions[0].account_id, ID);
    assert!(card.questions[0].keep.enabled);
}

#[test]
fn keeping_what_this_device_has_pauses_the_account_and_answers_the_question() {
    let app = signed_in("devices-keep", asked_about_alice());
    settled(&app).expect("asked");
    let page = app.account_page(ID.to_owned()).expect("listed");
    let sharing = page.sharing.expect("an IMAP account can be shared");
    assert_eq!(sharing.mode, AllodiaAccountSyncMode::On);
    assert!(sharing.control.enabled);

    // What the next pass answers once the question is answered.
    *app.allodia_sync.canned.lock().unwrap() = Some(Ok(AllodiaSyncReport::default()));
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SetSharing {
            account: ID.to_owned(),
            mode: AllodiaAccountSyncMode::Paused,
        },
    });
    assert!(
        settled(&app).expect("the pass answered").nothing_found,
        "nothing is left to ask"
    );
    let sharing = app
        .account_page(ID.to_owned())
        .expect("listed")
        .sharing
        .expect("shared");
    assert_eq!(sharing.mode, AllodiaAccountSyncMode::Paused);
    assert_eq!(sharing.failed, None);
}

#[test]
fn signing_out_takes_the_card_and_the_control_away() {
    let app = signed_in("devices-sign-out", asked_about_alice());
    settled(&app).expect("asked");
    app.sign_out_of_allodia().expect("signed out");
    assert_eq!(app.allodia_devices(), None);
    assert_eq!(
        app.account_page(ID.to_owned()).expect("listed").sharing,
        None
    );
}

#[test]
fn a_removed_account_leaves_the_card() {
    let app = signed_in("devices-remove", asked_about_alice());
    settled(&app).expect("asked");
    // Without a note the removal has no record to withdraw at the service, which a test never
    // asks.
    app.allodia_sync
        .bookkeeping()
        .expect("installed")
        .forget(ID)
        .expect("forgotten");
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::Remove {
            account: ID.to_owned(),
        },
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.account_page(ID.to_owned()).is_some() {
        assert!(Instant::now() < deadline, "the removal never ended");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        app.allodia_devices()
            .expect("answered")
            .questions
            .is_empty()
    );
}

/// An account whose server is reached at one name and verified as another is one the other
/// devices cannot hold, so its page offers no sharing to change.
#[test]
fn an_account_the_other_devices_cannot_hold_has_no_sharing_control() {
    let app = app_with(
        "devices-unshareable",
        &MAIL.replace("localhost:1", "127.0.0.1:1"),
    );
    sign_in(&app);
    *app.allodia_sync.canned.lock().unwrap() = Some(Ok(AllodiaSyncReport::default()));
    app.use_allodia_sync_state_store(Box::new(NoSyncState))
        .expect("the store is read");
    settled(&app);
    assert_eq!(
        app.account_page(ID.to_owned()).expect("listed").sharing,
        None
    );
}
