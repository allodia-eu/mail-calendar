//! Settings → Composing's default send account across the FFI, and that a new composer opens on
//! the account it names: one rule behind both, so a client draws the one and takes the other's
//! `account` as it is.

use std::sync::mpsc;

use super::*;
use crate::tests::{ChannelObserver, NullLogger};

fn showcase() -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    )
}

fn opened_from(app: &MailcalApp, composition: &str) -> String {
    let staging = crate::tests::temp_data_dir("send-account")
        .to_string_lossy()
        .into_owned();
    app.open_composer(
        composition.to_owned(),
        ComposeOrigin::New { account: None },
        staging,
        None,
    )
    .expect("the composer opens")
    .account
}

#[test]
fn the_setting_and_a_new_composer_name_the_same_account() {
    let app = showcase();
    let accounts = app.mailbox_list().accounts;
    let [first, second] = accounts.as_slice() else {
        panic!("the showcase seeds two accounts, got {}", accounts.len());
    };
    let choose = |selected: &str| SendAccountSetting::Choose {
        accounts: accounts
            .iter()
            .map(|row| SendAccountOption {
                id: row.id.clone(),
                email: row.email.clone(),
            })
            .collect(),
        selected: selected.to_owned(),
    };

    assert_eq!(app.send_account_setting(), choose(&first.id));
    assert_eq!(opened_from(&app, "before"), first.id);

    app.dispatch(Intent::Preferences {
        intent: PreferenceIntent::SetDefaultSendAccount {
            account: Some(second.id.clone()),
        },
    });
    assert_eq!(app.send_account_setting(), choose(&second.id));
    assert_eq!(opened_from(&app, "after"), second.id);
}

#[test]
fn without_an_account_there_is_nothing_to_choose() {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase_first_run(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    assert_eq!(app.send_account_setting(), SendAccountSetting::NoAccount);
}
