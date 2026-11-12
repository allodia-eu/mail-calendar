//! An account's page through `dispatch`: a change runs and its controls come back, one the core
//! refuses leaves a notice, and a use with no server or no grant says so before it is pressed.
//! Every server is a local port nothing listens on, so the account stays listed and never connects.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    AccountCapability, AccountChangeNotice, AccountCredentialStore, AccountPage, AccountPageIntent,
    AccountUseBlock, CredentialStoreError, Intent, LogLevel, MailcalApp, Surface,
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
    app_over(
        name,
        config,
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
}

fn app_over(name: &str, config: &str, store: Box<dyn AccountCredentialStore>) -> Arc<MailcalApp> {
    app_signalling(name, config, store, mpsc::channel().0)
}

fn app_signalling(
    name: &str,
    config: &str,
    store: Box<dyn AccountCredentialStore>,
    tx: mpsc::Sender<Surface>,
) -> Arc<MailcalApp> {
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config.to_owned()],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        store,
    )
    .expect("the app boots")
}

/// The page once no change runs any more.
fn settled(app: &MailcalApp) -> AccountPage {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // Asked of the work itself, not of the page's controls: a change to a use the page does
        // not list (colleagues, here) marks no control running. Asked before the page is read,
        // so the page cannot be one drawn while the change still ran.
        if !app.account_work.of(ID).0.busy {
            return app
                .account_page(ID.to_owned())
                .expect("the account is listed");
        }
        assert!(Instant::now() < deadline, "the change never ended");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The page once its mail half has been read, which it is in the background.
fn with_mail(app: &MailcalApp) -> AccountPage {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let page = settled(app);
        if page.mail.is_some() {
            return page;
        }
        assert!(Instant::now() < deadline, "the mail half was never read");
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

/// The signals `app` raised by the time a change dispatched now has ended.
fn signals_of(
    app: &MailcalApp,
    rx: &mpsc::Receiver<Surface>,
    intent: AccountPageIntent,
) -> Vec<Surface> {
    settled(app);
    // What booting raised is not this change's.
    std::thread::sleep(Duration::from_millis(200));
    rx.try_iter().for_each(drop);
    app.dispatch(Intent::AccountPage { intent });
    settled(app);
    std::thread::sleep(Duration::from_millis(200));
    rx.try_iter().collect()
}

#[test]
fn a_change_only_the_page_draws_moves_the_page_alone_and_one_stored_for_others_moves_settings() {
    let (tx, rx) = mpsc::channel();
    let app = app_signalling(
        "account-page-signals",
        WITH_CALENDAR,
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
        tx,
    );
    let page_only = signals_of(
        &app,
        &rx,
        AccountPageIntent::SetReplyFallback {
            account: ID.to_owned(),
            choice: crate::ReplyFallbackChoice::Always,
        },
    );
    let pages = |signals: &[Surface]| {
        signals
            .iter()
            .filter(|surface| matches!(surface, Surface::AccountPage))
            .count()
    };
    let settings = |signals: &[Surface]| {
        signals
            .iter()
            .any(|surface| matches!(surface, Surface::Settings))
    };
    // At least: a server the page asked how the account signs in may answer meanwhile.
    assert!(
        pages(&page_only) >= 2,
        "the page is redrawn as the change starts and as it ends"
    );
    assert!(!settings(&page_only), "nothing outside the page moved");

    let stored = signals_of(
        &app,
        &rx,
        AccountPageIntent::SetUse {
            account: ID.to_owned(),
            capability: AccountCapability::Calendar,
            on: false,
        },
    );
    assert!(pages(&stored) >= 2);
    assert!(settings(&stored), "the accounts every screen reads moved");
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
    assert_eq!(calendar.blocked, Some(AccountUseBlock::NeedsServer));
    assert!(!calendar.control.enabled);

    // Mail is the last use: the page holds it, and the core refuses it all the same.
    set_use(&app, AccountCapability::Mail, false);
    let notice = settled(&app).notice.expect("the refusal is said");
    assert!(matches!(
        notice.notice,
        AccountChangeNotice::UseFailed {
            capability: AccountCapability::Mail,
            on: false,
            refusal: crate::AccountChangeRefusal::Blocked {
                block: AccountUseBlock::LastUse
            },
        }
    ));

    set_use(&app, AccountCapability::Colleagues, true);
    let next = settled(&app).notice.expect("said again");
    assert!(
        next.id > notice.id,
        "each notice is told from the one before"
    );
}

#[test]
fn a_calendar_the_provider_refused_asks_it_again_on_the_page_and_in_the_change() {
    let app = app("account-page-calendar-refused", WITH_CALENDAR);
    settled(&app);
    app.app
        .note_calendar_reauth_required(&engine_api::AccountId::try_from(ID).unwrap());
    let calendar = settled(&app)
        .uses
        .into_iter()
        .find(|switch| switch.capability == AccountCapability::Calendar)
        .expect("listed");
    assert!(!calendar.on && calendar.asks_provider && calendar.control.enabled);

    // Pressed past the page, the core answers the same: nothing changes, and the provider is asked.
    set_use(&app, AccountCapability::Calendar, true);
    let notice = settled(&app).notice.expect("the switch did not take");
    assert!(matches!(
        notice.notice,
        AccountChangeNotice::UseFailed {
            capability: AccountCapability::Calendar,
            on: true,
            refusal: crate::AccountChangeRefusal::Withheld,
        }
    ));
}

#[test]
fn a_change_runs_in_the_showcase_too() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
        crate::ShowcaseLocale::En,
    );
    let account = app.accounts_snapshot().accounts[0].id.clone();
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SetLink {
            account: account.clone(),
            slot: crate::LinkSlot::Calendar,
            target: Some("nobody@nowhere.example".to_owned()),
        },
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let notice = loop {
        let page = app.account_page(account.clone()).expect("listed");
        if let Some(notice) = page.notice {
            break notice;
        }
        assert!(Instant::now() < deadline, "the change was never run");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(matches!(
        notice.notice,
        AccountChangeNotice::LinkFailed {
            slot: crate::LinkSlot::Calendar,
            ..
        }
    ));
}

/// A keystore that will not erase anything.
struct KeepingStore;

impl AccountCredentialStore for KeepingStore {
    fn persist(&self, _: String, _: String) -> Result<(), CredentialStoreError> {
        Ok(())
    }

    fn delete(&self, _: String) -> Result<(), CredentialStoreError> {
        Err(CredentialStoreError::Store("locked".to_owned()))
    }
}

/// Waits until the account's page is gone and the removal has ended, so what it came to is said.
fn removed(app: &MailcalApp) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.account_page(ID.to_owned()).is_some() || app.account_work.of(ID).0.busy {
        assert!(Instant::now() < deadline, "the account was never removed");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_removal_takes_the_page_and_says_on_the_list_what_it_could_not_finish() {
    let app = app("account-page-remove", MAIL_ONLY);
    settled(&app);
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::Remove {
            account: ID.to_owned(),
        },
    });
    removed(&app);
    assert_eq!(
        app.accounts_notice(),
        None,
        "a removal that worked says nothing"
    );

    // A notice left on the page goes with an account removed some other way.
    let elsewhere = self::app("account-page-remove-elsewhere", MAIL_ONLY);
    set_use(&elsewhere, AccountCapability::Mail, false);
    assert!(settled(&elsewhere).notice.is_some());
    elsewhere.remove_account(ID.to_owned()).expect("removed");
    assert_eq!(elsewhere.account_work.of(ID).1, None);

    let app = app_over(
        "account-page-remove-kept",
        MAIL_ONLY,
        Box::new(KeepingStore),
    );
    settled(&app);
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::Remove {
            account: ID.to_owned(),
        },
    });
    removed(&app);
    let notice = app.accounts_notice().expect("the list says it");
    assert!(
        matches!(notice.notice, AccountChangeNotice::RemoveFailed { .. }),
        "the account is gone, and the list says its credential was kept"
    );
}

/// The mail half is read at launch, before anything asks for the page, so the page's first read
/// is drawn whole rather than redrawn once the row arrives.
#[test]
fn the_mail_half_is_read_at_launch() {
    let app = app("account-page-mail-ahead", MAIL_ONLY);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.account_work.mail_rows.row(ID).is_none() {
        assert!(
            Instant::now() < deadline,
            "the row was never read at launch"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(settled(&app).mail.is_some());
}

/// A mail account's page carries its mail half, and a change to it runs through `dispatch` and is
/// on the page when it ends: the interval in effect, the name it sends under.
#[test]
fn a_mail_setting_changes_through_the_page() {
    let app = app("account-page-mail", MAIL_ONLY);
    let page = with_mail(&app);
    let mail = page.mail.expect("a mail account has its mail settings");
    // The server never answered, so nothing says it pushes: it checks on a timer.
    assert!(mail.push.is_none());
    let interval = mail
        .interval
        .expect("an account that cannot push checks on a timer");
    let other = interval
        .options
        .iter()
        .copied()
        .find(|minutes| *minutes != interval.options[interval.selected as usize])
        .expect("more than one interval is offered");

    // Asked for while the interval runs, as a field sends what was typed when the held page
    // takes its focus: it waits its turn rather than being turned away.
    for _ in 0..2 {
        app.dispatch(Intent::AccountPage {
            intent: AccountPageIntent::SetPollInterval {
                account: ID.to_owned(),
                minutes: other,
            },
        });
    }
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SetSenderName {
            account: ID.to_owned(),
            name: "Alice Example".to_owned(),
        },
    });
    let page = settled(&app);
    let interval = page
        .mail
        .and_then(|mail| mail.interval)
        .expect("still on a timer");
    assert_eq!(interval.options[interval.selected as usize], other);
    assert!(interval.control.enabled && !interval.control.running);
    let name = page.sender_name.expect("a mail account sends under a name");
    assert_eq!(name.name, "Alice Example");
}

/// An add the core refuses keeps the address in the field, as the person meant it, so it is
/// corrected rather than typed again.
#[test]
fn a_refused_add_keeps_the_address_it_was_given() {
    let app = app("account-page-shared-add", MAIL_ONLY);
    settled(&app);
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::AddSharedMailbox {
            account: ID.to_owned(),
            address: "  team@example.org ".to_owned(),
        },
    });
    let page = settled(&app);
    assert!(matches!(
        page.notice.map(|raised| raised.notice),
        Some(AccountChangeNotice::SharedMailboxFailed { .. })
    ));
    assert_eq!(
        app.account_work.shared.missed_at(ID).as_deref(),
        Some("team@example.org")
    );
}

/// The page, the accounts list and opening the page read only memory: a change to the account list
/// that a slow reader holds up never holds up the thread that draws them.
#[test]
fn reading_the_page_never_waits_on_the_account_list() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let app = app("account-page-unheld", MAIL_ONLY);
    with_mail(&app);

    let release = Arc::new(AtomicBool::new(false));
    let (held_tx, held_rx) = mpsc::channel();
    let holder = {
        let (app, release) = (Arc::clone(&app), Arc::clone(&release));
        std::thread::spawn(move || {
            let core = Arc::clone(&app.app);
            app.runtime.block_on(|| async move {
                core.hold_account_list(async move {
                    held_tx.send(()).expect("the test waits");
                    while !release.load(Ordering::Relaxed) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await;
            });
        })
    };
    held_rx.recv().expect("the list is held");

    let (read_tx, read_rx) = mpsc::channel();
    {
        let app = Arc::clone(&app);
        std::thread::spawn(move || {
            app.account_page_opened(ID.to_owned());
            let listed = app.accounts_snapshot().accounts.len();
            let page = app.account_page(ID.to_owned());
            read_tx.send((listed, page)).expect("the test waits");
        });
    }
    let read = read_rx.recv_timeout(Duration::from_secs(2));
    release.store(true, Ordering::Relaxed);
    holder.join().expect("the holder lets go");
    let (listed, page) = read.expect("read while the account list was held");
    assert_eq!(listed, 1);
    let page = page.expect("the account is listed");
    assert!(page.mail.is_some(), "drawn from what was last read");
}
