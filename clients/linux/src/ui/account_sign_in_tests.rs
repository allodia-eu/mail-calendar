use mailcal_bindings::AccountSignInOptions;

use super::{Outcome, Password, SignInSwitchState, Switch, settings_notice};
use crate::{l10n, ui::settings::notice::Notice};

const fn both() -> AccountSignInOptions {
    AccountSignInOptions {
        sign_in: true,
        provider_label: None,
        password: true,
    }
}

#[test]
fn a_page_opened_twice_asks_its_server_once_and_a_switch_asks_again() {
    let mut state = SignInSwitchState::default();
    assert!(state.ask("a"));
    assert!(!state.ask("a"), "the question is already out");
    assert_eq!(state.options("a"), None);

    state.answer("a".to_owned(), both());
    assert_eq!(state.options("a"), Some(&both()));
    assert_eq!(state.options("b"), None, "an answer is for its own account");
    assert!(!state.ask("a"), "the answer is in");

    // The way in changed, and the question depends on it.
    state.forget("a");
    assert_eq!(state.options("a"), None);
    assert!(state.ask("a"));
}

#[test]
fn forgetting_another_account_keeps_this_ones_answer() {
    let mut state = SignInSwitchState::default();
    state.answer("a".to_owned(), both());
    state.forget("b");
    assert_eq!(state.options("a"), Some(&both()));
}

#[test]
fn the_page_says_what_each_change_came_to_unless_the_browser_was_closed() {
    let toast = |text: &str| Some(Notice::Toast(text.to_owned()));
    assert_eq!(
        settings_notice(Switch::Again, &Outcome::Switched),
        toast(l10n::settings_account_signed_in())
    );
    assert_eq!(
        settings_notice(Switch::ToProvider, &Outcome::Switched),
        toast(l10n::settings_account_switched_to_provider())
    );
    assert_eq!(
        settings_notice(Switch::ToPassword, &Outcome::Switched),
        toast(l10n::settings_account_switched_to_password())
    );
    assert_eq!(
        settings_notice(Switch::ToProvider, &Outcome::Cancelled),
        None
    );

    let failed = Outcome::Failed(l10n::signin_refused().to_owned());
    assert_eq!(
        settings_notice(Switch::Again, &failed),
        Some(Notice::Error {
            title: l10n::settings_account_signin_failed().to_owned(),
            detail: l10n::signin_refused().to_owned(),
        })
    );
    for switch in [Switch::ToProvider, Switch::ToPassword] {
        assert_eq!(
            settings_notice(switch, &failed),
            Some(Notice::Error {
                title: l10n::settings_account_switch_failed().to_owned(),
                detail: l10n::signin_refused().to_owned(),
            })
        );
    }
}

#[test]
fn a_password_on_its_way_to_the_core_is_never_printed() {
    let password = Password("hunter2".to_owned());
    assert!(!format!("{password:?}").contains("hunter2"));
}
