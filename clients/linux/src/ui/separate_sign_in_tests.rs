//! When setup shows the step for a sign-in of its own, and what the form says and holds from the
//! core's section.

use mailcal_bindings::{
    AccountCapability, AccountChangeRefusal, AccountSeparateServer, ControlState,
};

use super::SeparateForm;
use crate::{
    l10n,
    ui::setup_state::{Phase, SetupState},
};

const MAILBOX: &str = "alice@example.test@imap.example.test";

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};

/// The section the core draws for `uses`, nothing running and nothing offered but a password.
pub(crate) fn section(uses: Vec<AccountCapability>) -> AccountSeparateServer {
    AccountSeparateServer {
        uses,
        host: "dav.example.test".to_owned(),
        username: "alice".to_owned(),
        sign_in: None,
        connect: FREE,
        stop_using: FREE,
        failed: None,
    }
}

pub(crate) fn form(uses: Vec<AccountCapability>) -> SeparateForm {
    SeparateForm::new(1, MAILBOX.to_owned(), section(uses), MAILBOX.to_owned())
}

#[test]
fn the_form_says_a_browser_failure_before_the_cores_and_nothing_otherwise() {
    let mut form = form(vec![AccountCapability::Contacts]);
    assert_eq!(form.said(), None);
    form.section.failed = Some(AccountChangeRefusal::SignInRefused);
    assert_eq!(form.said().as_deref(), Some(l10n::signin_refused()));
    form.failed(l10n::setup_imap_signin_failed().to_owned());
    assert_eq!(
        form.said().as_deref(),
        Some(l10n::setup_imap_signin_failed())
    );
}

#[test]
fn the_form_is_held_while_the_core_holds_connect() {
    let mut form = form(vec![AccountCapability::Calendar]);
    assert!(!form.held());
    form.section.connect = ControlState {
        enabled: false,
        running: true,
    };
    assert!(form.held(), "its own connect runs");
    form.section.connect = ControlState {
        enabled: false,
        running: false,
    };
    assert!(form.held(), "a change runs elsewhere");
}

#[test]
fn setup_shows_the_step_before_the_link_step_only_for_the_account_it_was_held_for() {
    let mut state = SetupState::closed();
    state.open(false);
    assert!(
        !state.show_separate(MAILBOX, MAILBOX),
        "an add whose calendar connected asks nothing"
    );

    state.hold_separate(MAILBOX, section(vec![AccountCapability::Calendar]));
    assert!(!state.show_separate("another@example.test", MAILBOX));
    assert!(state.show_separate(MAILBOX, "first@example.test"));
    assert_eq!(state.phase, Phase::Separate);
    assert!(
        !state.show_separate(MAILBOX, MAILBOX),
        "the step on screen is not shown again"
    );
    assert_eq!(
        state.leave_separate(),
        Some((MAILBOX.to_owned(), "first@example.test".to_owned())),
        "the link step it was shown in place of comes next"
    );
    assert!(state.separate.is_none());
    assert!(
        !state.show_separate(MAILBOX, MAILBOX),
        "once left, the link step follows"
    );
}

#[test]
fn a_new_flow_forgets_a_step_held_for_the_last_one() {
    let mut state = SetupState::closed();
    state.open(false);
    state.hold_separate(MAILBOX, section(vec![AccountCapability::Calendar]));
    state.open(false);
    assert!(!state.show_separate(MAILBOX, MAILBOX));
}
