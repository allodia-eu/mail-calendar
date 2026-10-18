//! When setup shows the step for a sign-in of its own, what it starts from, and what Skip
//! switches.

use mailcal_bindings::{
    AccountCapability, AccountEntry, AccountKind, AccountUse, AccountsSnapshot, CapabilityState,
    SeparateServer, UseProblem,
};

use super::{SeparateForm, server_of, skip_changes};
use crate::ui::setup_state::{Phase, SetupState};

const MAILBOX: &str = "alice@example.test@imap.example.test";

pub(crate) fn server(uses: Vec<AccountCapability>) -> SeparateServer {
    SeparateServer {
        uses,
        host: "dav.example.test".to_owned(),
        username: "alice".to_owned(),
    }
}

/// A mailbox whose calendar and contacts are on, with `problem` on the calendar, and the server
/// the core names for it when it has one.
pub(crate) fn mailbox(problem: Option<UseProblem>, server: Option<SeparateServer>) -> AccountEntry {
    let used = |capability, problem| AccountUse {
        capability,
        state: CapabilityState::On,
        problem,
    };
    AccountEntry {
        id: MAILBOX.to_owned(),
        address: "alice@example.test".to_owned(),
        kind: AccountKind::Imap,
        sign_in: mailcal_bindings::SignInMethod::Provider,
        uses: vec![
            used(AccountCapability::Mail, None),
            used(AccountCapability::Calendar, problem),
            used(AccountCapability::Contacts, None),
        ],
        links: mailcal_bindings::AccountLinksView::default(),
        linked_from: Vec::new(),
        link_candidates: mailcal_bindings::LinkCandidates::default(),
        endpoints: None,
        separate_server: server,
    }
}

pub(crate) fn form(uses: Vec<AccountCapability>) -> SeparateForm {
    SeparateForm::new(1, MAILBOX.to_owned(), server(uses), MAILBOX.to_owned())
}

#[test]
fn skip_switches_off_exactly_what_needs_the_sign_in() {
    assert_eq!(
        skip_changes(&server(vec![AccountCapability::Calendar])),
        [(AccountCapability::Calendar, false)]
    );
    assert_eq!(
        skip_changes(&server(vec![
            AccountCapability::Calendar,
            AccountCapability::Contacts
        ])),
        [
            (AccountCapability::Calendar, false),
            (AccountCapability::Contacts, false)
        ]
    );
}

#[test]
fn the_form_starts_from_the_login_the_core_holds_for_that_server() {
    let form = form(vec![AccountCapability::Contacts]);
    assert_eq!(form.username, "alice");
    assert!(!form.busy);
    assert!(form.error.is_none());
}

#[test]
fn the_server_is_read_from_the_account_it_is_for() {
    let mut snapshot = AccountsSnapshot {
        accounts: vec![mailbox(None, None)],
    };
    assert_eq!(server_of(&snapshot, MAILBOX), None);
    snapshot.accounts = vec![mailbox(
        Some(UseProblem::NeedsOwnSignIn),
        Some(server(vec![AccountCapability::Calendar])),
    )];
    assert_eq!(
        server_of(&snapshot, MAILBOX),
        Some(server(vec![AccountCapability::Calendar]))
    );
    assert_eq!(server_of(&snapshot, "someone-else"), None);
}

#[test]
fn setup_shows_the_step_before_the_link_step_only_for_the_account_it_was_held_for() {
    let mut state = SetupState::closed();
    state.open(false);
    assert!(
        !state.show_separate(MAILBOX, MAILBOX),
        "an add whose calendar connected asks nothing"
    );

    state.hold_separate(MAILBOX, server(vec![AccountCapability::Calendar]));
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
    state.hold_separate(MAILBOX, server(vec![AccountCapability::Calendar]));
    state.open(false);
    assert!(!state.show_separate(MAILBOX, MAILBOX));
}

#[test]
fn the_page_offers_its_action_under_the_last_use_that_needs_the_sign_in() {
    use crate::ui::separate_sign_in_view::action_after;
    assert_eq!(action_after(&mailbox(None, None)), None);
    assert_eq!(
        action_after(&mailbox(
            Some(UseProblem::SignInRefused),
            Some(server(vec![AccountCapability::Calendar]))
        )),
        Some(AccountCapability::Calendar)
    );
    assert_eq!(
        action_after(&mailbox(
            Some(UseProblem::NeedsOwnSignIn),
            Some(server(vec![
                AccountCapability::Calendar,
                AccountCapability::Contacts
            ]))
        )),
        Some(AccountCapability::Contacts)
    );
}
