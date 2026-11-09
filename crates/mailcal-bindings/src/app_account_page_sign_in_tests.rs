//! The Sign-in section through `dispatch`: a refused password said where it is typed again, and a
//! typed password tried as a change on the page, leaving a notice when it is not kept. The server
//! is a local port nothing listens on, so no password is ever kept.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    AccountChangeNotice, AccountPage, AccountPageIntent, AccountSignInLine, Intent, LogLevel,
    MailcalApp,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const MAIL_ONLY: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "alice@example.org"
password = "pw"
"#;

const ID: &str = "alice@example.org@localhost";

fn app(name: &str) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![MAIL_ONLY.to_owned()],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots")
}

fn settled(app: &MailcalApp) -> AccountPage {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if !app.account_work.of(ID).0.busy {
            return app
                .account_page(ID.to_owned())
                .expect("the account is listed");
        }
        assert!(Instant::now() < deadline, "the change never ended");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_refused_password_is_said_beside_the_servers_that_take_the_new_one() {
    let app = app("account-page-sign-in-refused");
    let page = settled(&app);
    assert!(!page.servers.as_ref().expect("editable").sign_in_refused);
    assert_eq!(
        page.sign_in.as_ref().map(|section| section.line),
        Some(AccountSignInLine::Password)
    );

    app.app
        .note_signin_expired(&engine_api::AccountId::try_from(ID).unwrap());
    let page = settled(&app);
    assert!(page.servers.expect("editable").sign_in_refused);
    let section = page.sign_in.expect("a section");
    // Its servers take the new password, so the section does not offer a second field.
    assert_eq!(
        (section.line, section.password),
        (AccountSignInLine::Password, None)
    );
}

#[test]
fn a_typed_password_is_never_printed() {
    let intent = AccountPageIntent::UsePassword {
        account: ID.to_owned(),
        password: crate::TypedPassword {
            text: "hunter2".to_owned(),
        },
    };
    assert!(!format!("{intent:?}").contains("hunter2"));
}

#[test]
fn a_password_that_does_not_connect_leaves_a_notice_and_the_account_as_it_was() {
    let app = app("account-page-sign-in-password");
    settled(&app);
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::UsePassword {
            account: ID.to_owned(),
            password: crate::TypedPassword {
                text: "new".to_owned(),
            },
        },
    });
    let page = settled(&app);
    assert!(matches!(
        page.notice.expect("said").notice,
        AccountChangeNotice::PasswordFailed { .. }
    ));
    assert_eq!(
        page.sign_in.map(|section| section.line),
        Some(AccountSignInLine::Password)
    );
}
