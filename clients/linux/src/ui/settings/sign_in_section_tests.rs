//! Which controls the Sign-in section of an IMAP or JMAP account offers, and what they send.
//!
//! The rules are plain `#[test]`s. The widget functions are called from the crate's single
//! `gtk::init` test, and assert on the **rendered** labels
//! ([`crate::ui::mailbox::tests::rendered_labels`]).

use adw::prelude::*;
use mailcal_bindings::{AccountKind, AccountSignInOptions, SignInMethod};

use super::{SignInControl, draw, label, section};
use crate::{
    l10n,
    ui::{
        AppInput,
        account_settings::AccountsInput,
        account_sign_in::{SignInInput, Stage, Switch},
        mailbox::tests::{every_row_belongs_to_a_list, rendered_labels},
        setup_widget_tests::descendants,
    },
};

const ACCOUNT: &str = "alice@example.org@imap.example.org";

const fn options(sign_in: bool, password: bool) -> AccountSignInOptions {
    AccountSignInOptions {
        sign_in,
        provider_label: None,
        password,
    }
}

#[test]
fn microsoft_google_and_calendar_accounts_have_no_such_section() {
    for kind in [
        AccountKind::Microsoft,
        AccountKind::Google,
        AccountKind::Dav,
    ] {
        let both = options(true, true);
        assert_eq!(section(kind, SignInMethod::Password, Some(&both)), None);
        assert_eq!(section(kind, SignInMethod::Provider, Some(&both)), None);
    }
}

#[test]
fn before_the_server_answers_the_section_says_only_how_it_signs_in() {
    let password = section(AccountKind::Imap, SignInMethod::Password, None).expect("a section");
    assert_eq!(password.status, l10n::settings_account_signs_in_password());
    assert_eq!(password.controls, []);
    assert!(!password.switches());
    let provider = section(AccountKind::Jmap, SignInMethod::Provider, None).expect("a section");
    assert_eq!(provider.status, l10n::settings_account_signs_in_provider());
    assert_eq!(provider.controls, []);
}

#[test]
fn a_signed_in_account_signs_in_again_and_switches_to_a_password_its_server_takes() {
    let both = section(
        AccountKind::Imap,
        SignInMethod::Provider,
        Some(&options(true, true)),
    )
    .expect("a section");
    assert_eq!(
        both.controls,
        [SignInControl::SignInAgain, SignInControl::ToPassword]
    );
    assert!(both.switches());

    // A server that names no password mechanism is only signed in again, and nothing is
    // switched, so the line about what a switch keeps does not belong.
    let again = section(
        AccountKind::Imap,
        SignInMethod::Provider,
        Some(&options(true, false)),
    )
    .expect("a section");
    assert_eq!(again.controls, [SignInControl::SignInAgain]);
    assert!(!again.switches());
}

#[test]
fn a_password_account_switches_to_signing_in_where_its_server_offers_it() {
    let offered = AccountSignInOptions {
        provider_label: Some("Fastmail".to_owned()),
        ..options(true, true)
    };
    let switch =
        section(AccountKind::Jmap, SignInMethod::Password, Some(&offered)).expect("a section");
    assert_eq!(
        switch.controls,
        [SignInControl::ToProvider(Some("Fastmail".to_owned()))]
    );
    assert!(switch.switches());
    assert_eq!(
        label(&switch.controls[0]),
        l10n::settings_account_switch_to_provider_named("Fastmail")
    );
    assert_eq!(
        label(&SignInControl::ToProvider(None)),
        l10n::settings_account_switch_to_provider()
    );

    let password_only = section(
        AccountKind::Imap,
        SignInMethod::Password,
        Some(&options(false, true)),
    )
    .expect("a section");
    assert_eq!(password_only.controls, []);
}

fn rendered(group: &adw::PreferencesGroup) -> Vec<String> {
    rendered_labels(group.upcast_ref())
}

/// The section draws its controls in plain text, every row reachable from the keyboard, and the
/// line saying what a switch keeps beneath them.
pub(crate) fn the_section_draws_the_switch_its_server_takes() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let offered = AccountSignInOptions {
        provider_label: Some("Mail & Co".to_owned()),
        ..options(true, true)
    };
    let switch =
        section(AccountKind::Imap, SignInMethod::Password, Some(&offered)).expect("a section");
    let group = draw(&switch, ACCOUNT, ACCOUNT, None, &sender);
    every_row_belongs_to_a_list(group.upcast_ref());
    let labels = rendered(&group);
    assert!(labels.contains(&l10n::settings_account_signs_in_password().to_owned()));
    assert!(
        labels.contains(&l10n::settings_account_switch_to_provider_named(
            "Mail & Co"
        )),
        "a provider's name is not markup: {labels:?}"
    );
    assert!(labels.contains(&l10n::settings_account_switch_description().to_owned()));

    let row = descendants::<adw::ButtonRow>(group.upcast_ref())
        .into_iter()
        .next()
        .expect("the switch's row");
    row.emit_by_name::<()>("activated", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::Accounts(AccountsInput::SignIn(SignInInput::Browser {
            account,
            switch: Switch::ToProvider,
        }))) if account == ACCOUNT
    ));
}

/// "Use a password instead" reveals an empty field; the password goes to the core once, and the
/// field is emptied as it goes.
pub(crate) fn a_password_is_typed_into_an_empty_field_and_cleared_once_sent() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let signed_in = section(
        AccountKind::Imap,
        SignInMethod::Provider,
        Some(&options(true, true)),
    )
    .expect("a section");
    let group = draw(&signed_in, ACCOUNT, ACCOUNT, None, &sender);
    every_row_belongs_to_a_list(group.upcast_ref());
    let rows = descendants::<adw::ButtonRow>(group.upcast_ref());
    let titled = |title: &str| {
        rows.iter()
            .find(|row| row.title() == title)
            .cloned()
            .expect("a row with that title")
    };
    let again = titled(l10n::signin_expired_action());
    let reveal = titled(l10n::settings_account_switch_to_password());
    let save = titled(l10n::settings_account_switch_password_save());
    let field = descendants::<adw::PasswordEntryRow>(group.upcast_ref())
        .into_iter()
        .next()
        .expect("a password field");
    assert!(!field.is_visible() && !save.is_visible());

    reveal.emit_by_name::<()>("activated", &[]);
    assert!(field.is_visible() && save.is_visible() && !reveal.is_visible());
    assert!(
        field.text().is_empty(),
        "the current credential is never shown"
    );

    // Nothing typed, nothing sent.
    save.emit_by_name::<()>("activated", &[]);
    field.set_text("hunter2");
    save.emit_by_name::<()>("activated", &[]);
    match receiver.recv_sync() {
        Some(AppInput::Accounts(AccountsInput::SignIn(SignInInput::Password {
            account,
            password,
        }))) => {
            assert_eq!(account, ACCOUNT);
            assert_eq!(password.0, "hunter2");
            assert!(!format!("{password:?}").contains("hunter2"));
        }
        other => panic!("expected the password switch, got {other:?}"),
    }
    assert!(field.text().is_empty(), "the field is emptied once sent");

    again.emit_by_name::<()>("activated", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::Accounts(AccountsInput::SignIn(
            SignInInput::Browser {
                switch: Switch::Again,
                ..
            }
        )))
    ));
}

/// While the core works on a change, its controls wait.
pub(crate) fn a_running_change_holds_every_control() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let signed_in = section(
        AccountKind::Jmap,
        SignInMethod::Provider,
        Some(&options(true, true)),
    )
    .expect("a section");
    let group = draw(&signed_in, ACCOUNT, ACCOUNT, Some(Stage::Core), &sender);
    let rows = descendants::<adw::ButtonRow>(group.upcast_ref());
    assert!(
        rows.iter()
            .filter(|row| row.is_visible())
            .all(|row| !row.is_sensitive())
    );
    assert!(
        descendants::<gtk::Button>(group.upcast_ref())
            .iter()
            .all(|button| button.label().as_deref() != Some(l10n::action_cancel()))
    );
}

/// While the browser holds a change, nobody is stranded: every control stays live, so a fresh
/// press replaces the attempt, and Cancel beside the line saying where it waits drops it.
pub(crate) fn a_change_waiting_on_the_browser_can_be_replaced_or_cancelled() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let signed_in = section(
        AccountKind::Imap,
        SignInMethod::Provider,
        Some(&options(true, true)),
    )
    .expect("a section");
    let group = draw(
        &signed_in,
        ACCOUNT,
        "alice@example.org",
        Some(Stage::Browser),
        &sender,
    );
    every_row_belongs_to_a_list(group.upcast_ref());
    assert!(rendered(&group).contains(&l10n::settings_account_signin_waiting("alice@example.org")));
    let rows = descendants::<adw::ButtonRow>(group.upcast_ref());
    assert!(
        rows.iter()
            .filter(|row| row.is_visible())
            .all(gtk::prelude::WidgetExt::is_sensitive)
    );
    let again = rows
        .iter()
        .find(|row| row.title() == l10n::signin_expired_action())
        .expect("sign in again");
    again.emit_by_name::<()>("activated", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::Accounts(AccountsInput::SignIn(
            SignInInput::Browser {
                switch: Switch::Again,
                ..
            }
        )))
    ));

    let cancel = descendants::<gtk::Button>(group.upcast_ref())
        .into_iter()
        .find(|button| button.label().as_deref() == Some(l10n::action_cancel()))
        .expect("a Cancel");
    cancel.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::Accounts(AccountsInput::SignIn(SignInInput::Cancel { account })))
            if account == ACCOUNT
    ));
}
