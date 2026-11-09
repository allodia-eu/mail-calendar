use mailcal_account::{AccountLinks, Capability};

use super::{Failure, Offer, SeparateNeed, SeparateState, need, section};
use crate::{
    AccountCapability, AccountChangeRefusal, AccountSeparateSignIn, ControlState,
    account_work::{Hold, PageWork},
    accounts_view::{
        DavServer,
        tests::{STANDARDS, mailbox, set},
    },
};

const BOTH: [AccountCapability; 2] = [AccountCapability::Calendar, AccountCapability::Contacts];

fn at(login: &str) -> crate::accounts_view::AccountFacts {
    let mut signed_in = mailbox(AccountLinks::default());
    signed_in.chosen = set(&STANDARDS);
    signed_in.dav_servers = [Capability::Calendar, Capability::Contacts]
        .into_iter()
        .map(|capability| DavServer {
            capability,
            base_url: "https://dav.example.org:8443/alice@example.org/".to_owned(),
            username: login.to_owned(),
        })
        .collect();
    signed_in
}

fn both(problem: mailcal_app::UseProblem) -> mailcal_app::UseProblems {
    mailcal_app::UseProblems {
        calendar: Some(problem),
        contacts: Some(problem),
    }
}

fn needed() -> SeparateNeed {
    SeparateNeed {
        uses: BOTH.to_vec(),
        host: "dav.example.org".to_owned(),
        username: "alice".to_owned(),
    }
}

fn free() -> Hold {
    Hold::default()
}

fn refused() -> Failure {
    Failure {
        username: "alice2".to_owned(),
        refusal: AccountChangeRefusal::SignInRefused,
    }
}

/// A mailbox that signs in through its provider, with calendar and contacts at a server the
/// grant does not reach: the form names that server's host, never its path, and the login the
/// mailbox holds there.
#[test]
fn a_calendar_that_needs_a_sign_in_of_its_own_names_its_server() {
    let found = need(
        &at("alice@example.org"),
        both(mailcal_app::UseProblem::NeedsOwnSignIn),
    )
    .expect("the form is offered");
    assert_eq!(found.uses, BOTH);
    assert_eq!(found.host, "dav.example.org:8443");
    assert_eq!(found.username, "alice@example.org");

    let refused = need(&at("alice"), both(mailcal_app::UseProblem::SignInRefused))
        .expect("a refused sign-in is offered one of its own too");
    assert_eq!(refused.username, "alice");

    let unreachable = mailcal_app::UseProblems {
        calendar: Some(mailcal_app::UseProblem::Unreachable),
        contacts: None,
    };
    assert_eq!(
        need(&at("alice"), unreachable),
        None,
        "a server that could not be reached is tried again, not signed in to separately"
    );
}

#[test]
fn a_server_with_no_login_starts_the_form_from_the_mailbox_address() {
    let account = at(" ");
    let found = need(&account, both(mailcal_app::UseProblem::NeedsOwnSignIn)).expect("offered");
    assert_eq!(found.username, account.address);
}

#[test]
fn only_an_answer_about_these_uses_offers_a_way_in() {
    let provider = Offer {
        uses: BOTH.to_vec(),
        sign_in: Some(AccountSeparateSignIn::Provider),
    };
    assert_eq!(
        section(needed(), Some(&provider), None, &free()).sign_in,
        Some(AccountSeparateSignIn::Provider)
    );
    let calendar_only = Offer {
        uses: vec![AccountCapability::Calendar],
        ..provider
    };
    assert_eq!(
        section(needed(), Some(&calendar_only), None, &free()).sign_in,
        None
    );
    assert_eq!(section(needed(), None, None, &free()).sign_in, None);
}

#[test]
fn a_running_connect_is_drawn_with_its_login_and_hides_the_last_failure() {
    let idle = section(needed(), None, Some(&refused()), &free());
    assert_eq!(
        idle.username, "alice2",
        "the login that was refused, to correct"
    );
    assert_eq!(idle.failed, Some(AccountChangeRefusal::SignInRefused));
    assert_eq!(
        idle.connect,
        ControlState {
            enabled: true,
            running: false
        }
    );

    let connecting = Hold {
        busy: true,
        own: vec![PageWork::SeparateServer {
            username: "alice3".to_owned(),
        }],
    };
    let running = section(needed(), None, Some(&refused()), &connecting);
    assert_eq!(running.username, "alice3");
    assert_eq!(running.failed, None);
    assert_eq!(
        running.connect,
        ControlState {
            enabled: false,
            running: true
        }
    );
    assert!(!running.stop_using.running && !running.stop_using.enabled);
}

#[test]
fn skip_runs_on_its_own_control_and_a_change_elsewhere_holds_both() {
    let stopping = Hold {
        busy: true,
        own: vec![PageWork::StopSeparate],
    };
    let drawn = section(needed(), None, Some(&refused()), &stopping);
    assert!(drawn.stop_using.running && !drawn.connect.running);
    assert_eq!(drawn.failed, None);
    assert_eq!(drawn.username, "alice");

    let elsewhere = Hold {
        busy: true,
        own: Vec::new(),
    };
    let held = section(needed(), None, None, &elsewhere);
    assert!(!held.connect.enabled && !held.connect.running);
    assert!(!held.stop_using.enabled && !held.stop_using.running);
}

#[test]
fn an_attempt_that_worked_or_an_account_that_went_leaves_no_failure() {
    let state = SeparateState::default();
    state.tried("a", Some(refused()));
    assert!(state.section("a", needed(), &free()).failed.is_some());
    state.tried("a", None);
    assert!(state.section("a", needed(), &free()).failed.is_none());

    state.tried("a", Some(refused()));
    let ticket = state.offers.ask("a", false).expect("asked");
    state.forget("a");
    state.offers.answered(
        "a",
        ticket,
        Offer {
            uses: BOTH.to_vec(),
            sign_in: Some(AccountSeparateSignIn::Nextcloud),
        },
    );
    let drawn = state.section("a", needed(), &free());
    assert_eq!(drawn.failed, None);
    assert_eq!(
        drawn.sign_in, None,
        "an ask out for an account that went is dropped"
    );
}
