//! How the Sign-in section words and draws the core's record, and what each control sends.
//!
//! The wording is plain `#[test]`s. The widget functions are called from the crate's single
//! `gtk::init` test, and assert on the **rendered** labels
//! ([`crate::ui::mailbox::tests::rendered_labels`]).

use adw::prelude::*;
use mailcal_bindings::{
    AccountKind, AccountPageIntent, AccountPasswordField, AccountSecretKind, AccountSignIn,
    AccountSignInLine, AccountSignInRoute, ControlState,
};

use super::{draw, label, line};
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

const IDLE: ControlState = ControlState {
    enabled: true,
    running: false,
};

fn section(
    line: AccountSignInLine,
    routes: Vec<AccountSignInRoute>,
    password: Option<AccountPasswordField>,
) -> AccountSignIn {
    AccountSignIn {
        line,
        switches: password.is_some_and(|field| !field.replaces)
            || routes
                .iter()
                .any(|route| matches!(route, AccountSignInRoute::ToProvider { .. })),
        routes,
        control: IDLE,
        password,
    }
}

const fn field(replaces: bool, save: ControlState) -> AccountPasswordField {
    AccountPasswordField {
        kind: if replaces {
            AccountSecretKind::PasswordOrToken
        } else {
            AccountSecretKind::Password
        },
        replaces,
        save,
    }
}

fn signed_in() -> AccountSignIn {
    section(
        AccountSignInLine::Provider,
        vec![AccountSignInRoute::SignInAgain],
        Some(field(false, IDLE)),
    )
}

#[test]
fn each_line_and_route_is_worded() {
    assert_eq!(
        line(AccountSignInLine::Refused, "alice@example.org"),
        l10n::signin_expired_prompt("alice@example.org")
    );
    assert_eq!(
        line(AccountSignInLine::NeedsPermission, "alice@example.org"),
        l10n::settings_account_needs_permission()
    );
    assert_eq!(
        line(AccountSignInLine::Nextcloud, "alice@example.org"),
        l10n::settings_account_signs_in_nextcloud()
    );
    assert_eq!(
        label(&AccountSignInRoute::ToProvider { label: None }),
        l10n::settings_account_switch_to_provider()
    );
    assert_eq!(
        label(&AccountSignInRoute::ToNextcloud),
        l10n::settings_account_switch_to_provider_named("Nextcloud")
    );
}

fn rendered(group: &adw::PreferencesGroup) -> Vec<String> {
    rendered_labels(group.upcast_ref())
}

fn browser(input: Option<AppInput>) -> Option<(String, Switch)> {
    match input {
        Some(AppInput::Accounts(AccountsInput::SignIn(SignInInput::Browser {
            account,
            switch,
        }))) => Some((account, switch)),
        _ => None,
    }
}

/// The section draws its routes in plain text, every row reachable from the keyboard, and the
/// line saying what a switch keeps beneath them.
pub(crate) fn the_section_draws_the_switch_its_server_takes() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let switch = section(
        AccountSignInLine::Password,
        vec![AccountSignInRoute::ToProvider {
            label: Some("Mail & Co".to_owned()),
        }],
        None,
    );
    let group = draw(&switch, AccountKind::Imap, ACCOUNT, ACCOUNT, None, &sender);
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
    assert_eq!(
        browser(receiver.recv_sync()),
        Some((ACCOUNT.to_owned(), Switch::ToProvider))
    );
}

/// A calendar account at a Nextcloud server draws the switch to signing in with Nextcloud, which
/// sends the browser sign-in for it; a Microsoft account signs in again through its consent.
pub(crate) fn each_route_goes_to_the_sign_in_its_account_takes() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let typed = section(
        AccountSignInLine::Password,
        vec![AccountSignInRoute::ToNextcloud],
        None,
    );
    let group = draw(&typed, AccountKind::Dav, ACCOUNT, ACCOUNT, None, &sender);
    every_row_belongs_to_a_list(group.upcast_ref());
    assert!(!rendered(&group).contains(&l10n::settings_account_switch_description().to_owned()));
    descendants::<adw::ButtonRow>(group.upcast_ref())[0].emit_by_name::<()>("activated", &[]);
    assert_eq!(
        browser(receiver.recv_sync()),
        Some((ACCOUNT.to_owned(), Switch::ToProvider))
    );

    let consent = section(
        AccountSignInLine::AtProvider,
        vec![AccountSignInRoute::SignInAgain],
        None,
    );
    let group = draw(
        &consent,
        AccountKind::Microsoft,
        ACCOUNT,
        ACCOUNT,
        None,
        &sender,
    );
    assert!(rendered(&group).contains(&l10n::settings_account_signin_description().to_owned()));
    descendants::<adw::ButtonRow>(group.upcast_ref())[0].emit_by_name::<()>("activated", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::Accounts(AccountsInput::SignInAgain { account, adding }))
            if account == ACCOUNT && adding.is_empty()
    ));
}

fn password_entry(group: &adw::PreferencesGroup) -> adw::PasswordEntryRow {
    descendants::<adw::PasswordEntryRow>(group.upcast_ref())
        .into_iter()
        .next()
        .expect("a password field")
}

fn button(group: &adw::PreferencesGroup, label: &str) -> gtk::Button {
    descendants::<gtk::Button>(group.upcast_ref())
        .into_iter()
        .find(|button| rendered_labels(button.upcast_ref()).contains(&label.to_owned()))
        .expect("a button with that label")
}

fn sent_password(input: Option<AppInput>) -> Option<(String, String)> {
    match input {
        Some(AppInput::Accounts(AccountsInput::Change(AccountPageIntent::UsePassword {
            account,
            password,
        }))) => {
            assert!(!format!("{password:?}").contains(&password.text));
            Some((account, password.text))
        }
        _ => None,
    }
}

/// "Use a password instead" reveals an empty field; the password goes to the core once, as a
/// change on the page, and the field is emptied as it goes.
pub(crate) fn a_password_is_typed_into_an_empty_field_and_cleared_once_sent() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let group = draw(
        &signed_in(),
        AccountKind::Imap,
        ACCOUNT,
        ACCOUNT,
        None,
        &sender,
    );
    every_row_belongs_to_a_list(group.upcast_ref());
    let reveal = descendants::<adw::ButtonRow>(group.upcast_ref())
        .into_iter()
        .find(|row| row.title() == l10n::settings_account_switch_to_password())
        .expect("the reveal");
    let field = password_entry(&group);
    let save = button(&group, l10n::settings_account_switch_password_save());
    assert!(!field.is_visible());

    reveal.emit_by_name::<()>("activated", &[]);
    assert!(field.is_visible() && !reveal.is_visible());
    assert!(
        field.text().is_empty(),
        "the current credential is never shown"
    );
    assert!(!save.is_sensitive(), "nothing typed, nothing to send");

    field.set_text("hunter2");
    assert!(save.is_sensitive());
    save.emit_clicked();
    assert_eq!(
        sent_password(receiver.recv_sync()),
        Some((ACCOUNT.to_owned(), "hunter2".to_owned()))
    );
    assert!(field.text().is_empty(), "the field is emptied once sent");
}

/// A password the server refused is typed again at once, a token as well on a JMAP account.
pub(crate) fn a_refused_password_is_typed_again_where_the_section_says_so() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let refused = section(
        AccountSignInLine::Refused,
        Vec::new(),
        Some(field(true, IDLE)),
    );
    let group = draw(
        &refused,
        AccountKind::Jmap,
        ACCOUNT,
        "alice@example.org",
        None,
        &sender,
    );
    every_row_belongs_to_a_list(group.upcast_ref());
    assert!(rendered(&group).contains(&l10n::signin_expired_prompt("alice@example.org")));
    let field = password_entry(&group);
    assert!(field.is_visible());
    assert_eq!(field.title(), l10n::setup_jmap_secret_placeholder());
    field.set_text("token");
    field.emit_by_name::<()>("entry-activated", &[]);
    assert_eq!(
        sent_password(receiver.recv_sync()),
        Some((ACCOUNT.to_owned(), "token".to_owned()))
    );
}

/// While a password is tried its field stays open and its button spins; while the core completes
/// a browser sign-in, every route waits.
pub(crate) fn a_running_change_holds_every_control() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut trying = signed_in();
    trying.control = ControlState {
        enabled: false,
        running: false,
    };
    trying.password = Some(field(
        false,
        ControlState {
            enabled: false,
            running: true,
        },
    ));
    let group = draw(&trying, AccountKind::Imap, ACCOUNT, ACCOUNT, None, &sender);
    let field = password_entry(&group);
    assert!(field.is_visible() && !field.is_sensitive());
    assert_ne!(descendants::<adw::Spinner>(group.upcast_ref()).len(), 0);

    let group = draw(
        &signed_in(),
        AccountKind::Imap,
        ACCOUNT,
        ACCOUNT,
        Some(Stage::Core),
        &sender,
    );
    assert!(
        descendants::<adw::ButtonRow>(group.upcast_ref())
            .iter()
            .filter(|row| row.is_visible())
            .all(|row| !row.is_sensitive())
    );
}

/// While the browser holds a change, nobody is stranded: every route stays live, so a fresh press
/// replaces the attempt, and Cancel beside the line saying where it waits drops it.
pub(crate) fn a_change_waiting_on_the_browser_can_be_replaced_or_cancelled() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let group = draw(
        &signed_in(),
        AccountKind::Imap,
        ACCOUNT,
        "alice@example.org",
        Some(Stage::Browser),
        &sender,
    );
    every_row_belongs_to_a_list(group.upcast_ref());
    assert!(rendered(&group).contains(&l10n::settings_account_signin_waiting("alice@example.org")));
    let again = descendants::<adw::ButtonRow>(group.upcast_ref())
        .into_iter()
        .find(|row| row.title() == l10n::signin_expired_action())
        .expect("sign in again");
    assert!(again.is_sensitive());
    again.emit_by_name::<()>("activated", &[]);
    assert_eq!(
        browser(receiver.recv_sync()),
        Some((ACCOUNT.to_owned(), Switch::Again))
    );

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
