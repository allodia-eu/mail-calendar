//! An account's page through `dispatch`: a change runs and its controls come back, one the core
//! refuses leaves a notice, and a use with no server says so before it is pressed. Every server is
//! a local port nothing listens on, so the account stays listed and never connects.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    AccountCapability, AccountChangeNotice, AccountPage, AccountPageIntent, AccountUseNote, Intent,
    LogLevel, MailcalApp,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const WITH_CALENDAR: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "alice@example.org"
password = "pw"

[caldav]
base_url = "http://127.0.0.1:1/dav"
username = "alice@example.org"
password = "pw"
"#;

const MAIL_ONLY: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "alice@example.org"
password = "pw"
"#;

const ID: &str = "alice@example.org@localhost";

fn app(name: &str, config: &str) -> Arc<MailcalApp> {
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

/// The page once nothing runs on it any more.
fn settled(app: &MailcalApp) -> AccountPage {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let page = app
            .account_page(ID.to_owned())
            .expect("the account is listed");
        if !page.remove.running && page.uses.iter().all(|switch| !switch.control.running) {
            return page;
        }
        assert!(Instant::now() < deadline, "the change never ended");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn set_use(app: &MailcalApp, capability: AccountCapability, on: bool) {
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SetUse {
            account: ID.to_owned(),
            capability,
            on,
        },
    });
}

#[test]
fn a_switch_runs_and_the_page_shows_what_it_came_to() {
    let app = app("account-page-switch", WITH_CALENDAR);
    let calendar = |page: &AccountPage| {
        page.uses
            .iter()
            .find(|switch| switch.capability == AccountCapability::Calendar)
            .cloned()
            .expect("listed")
    };
    assert!(calendar(&settled(&app)).on);

    set_use(&app, AccountCapability::Calendar, false);
    let page = settled(&app);
    assert!(!calendar(&page).on);
    assert!(calendar(&page).control.enabled, "the page is free again");
    assert_eq!(page.notice, None, "a change that worked says nothing");
}

#[test]
fn a_change_the_core_refuses_leaves_a_notice_and_a_missing_server_is_said_first() {
    let app = app("account-page-refused", MAIL_ONLY);
    let page = settled(&app);
    let calendar = page
        .uses
        .iter()
        .find(|switch| switch.capability == AccountCapability::Calendar)
        .expect("listed");
    assert_eq!(calendar.note, Some(AccountUseNote::NeedsServer));
    assert!(!calendar.control.enabled);

    // Mail is the last use: the page holds it, and the core refuses it all the same.
    set_use(&app, AccountCapability::Mail, false);
    let notice = settled(&app).notice.expect("the refusal is said");
    assert!(matches!(
        notice.notice,
        AccountChangeNotice::UseFailed {
            capability: AccountCapability::Mail,
            on: false,
            ..
        }
    ));

    set_use(&app, AccountCapability::Colleagues, true);
    let next = settled(&app).notice.expect("said again");
    assert!(
        next.id > notice.id,
        "each notice is told from the one before"
    );
}
