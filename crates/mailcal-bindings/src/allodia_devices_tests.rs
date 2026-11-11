//! The other devices' card as the core keeps it: a pass asked for while one runs is run again
//! rather than lost, one that ends after its sign-in has gone says nothing, and what a sharing
//! change or a removal answers leaves the card.

use super::{AllodiaElsewhere, AllodiaSyncFailure, Devices};
use crate::{
    ControlState,
    allodia_sync::{
        AllodiaAccountChange, AllodiaAccountKind, AllodiaAccountOffer, AllodiaSyncReport,
    },
};

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};

fn change(account: &str) -> AllodiaAccountChange {
    AllodiaAccountChange {
        account_id: account.to_owned(),
        email: format!("{account}@test.local"),
        also_changed_here: false,
    }
}

fn offer() -> AllodiaAccountOffer {
    AllodiaAccountOffer {
        id: "record".to_owned(),
        email: "arriving@test.local".to_owned(),
        kind: AllodiaAccountKind::Google,
        host: None,
        port: None,
        security: None,
        smtp_host: None,
        smtp_port: None,
        smtp_security: None,
        caldav_base_url: None,
        jmap_base_url: None,
    }
}

fn report() -> AllodiaSyncReport {
    AllodiaSyncReport {
        offers: vec![offer()],
        changed_elsewhere: vec![change("moved")],
        removed_elsewhere: vec![change("gone")],
        sent: 0,
    }
}

#[test]
fn nothing_is_known_before_a_pass_has_answered_and_one_that_found_nothing_says_so() {
    let devices = Devices::default();
    assert_eq!(devices.view(|_| FREE), None);

    let session = devices.want_pass().expect("nothing runs yet");
    let checking = devices.view(|_| FREE).expect("a pass runs");
    assert!(checking.checking);

    assert!(!checking.nothing_found, "it has not answered yet");

    assert_eq!(
        devices.pass_ended(session, Ok(AllodiaSyncReport::default())),
        None
    );
    let found = devices.view(|_| FREE).expect("a pass answered");
    assert!(found.nothing_found, "a pass that found nothing says so");
}

#[test]
fn a_pass_answers_with_offers_then_changed_then_removed() {
    let devices = Devices::default();
    let session = devices.want_pass().expect("nothing runs yet");
    devices.pass_ended(session, Ok(report()));

    let card = devices.view(|_| FREE).expect("something to say");
    assert!(!card.checking);
    assert!(!card.nothing_found);
    assert_eq!(card.offers, vec![offer()]);
    let asked: Vec<_> = card
        .questions
        .iter()
        .map(|question| (question.account_id.as_str(), question.elsewhere))
        .collect();
    assert_eq!(
        asked,
        [
            ("moved", AllodiaElsewhere::Changed),
            ("gone", AllodiaElsewhere::Removed)
        ]
    );
}

#[test]
fn a_pass_asked_for_while_one_runs_runs_once_more_after_it() {
    let devices = Devices::default();
    let session = devices.want_pass().expect("nothing runs yet");
    assert_eq!(devices.want_pass(), None, "one runs");
    assert_eq!(devices.want_pass(), None, "still one more, not two");

    let next = devices.pass_ended(session, Ok(AllodiaSyncReport::default()));
    assert_eq!(next, Some(session), "the one asked for meanwhile");
    assert!(!devices.view(|_| FREE).expect("it runs").nothing_found);
    assert!(devices.view(|_| FREE).expect("it runs").checking);
    assert_eq!(devices.pass_ended(session, Ok(report())), None);
    assert!(!devices.view(|_| FREE).expect("it found something").checking);
}

#[test]
fn a_pass_that_ends_after_a_sign_out_says_nothing() {
    let devices = Devices::default();
    let session = devices.want_pass().expect("nothing runs yet");
    devices.new_session();
    assert_eq!(devices.pass_ended(session, Ok(report())), None);
    assert_eq!(
        devices.view(|_| FREE),
        None,
        "nothing is known about the next sign-in"
    );
}

#[test]
fn a_sign_in_during_a_pass_runs_one_for_the_new_account() {
    let devices = Devices::default();
    let session = devices.want_pass().expect("nothing runs yet");
    devices.new_session();
    assert_eq!(devices.want_pass(), None, "the old one still runs");

    let next = devices
        .pass_ended(session, Ok(report()))
        .expect("the new sign-in's pass");
    assert_ne!(next, session);
    devices.pass_ended(next, Ok(report()));
    assert_eq!(devices.view(|_| FREE).expect("found").offers.len(), 1);
}

#[test]
fn a_failed_pass_is_said_and_the_next_one_clears_it() {
    let devices = Devices::default();
    let session = devices.want_pass().expect("nothing runs yet");
    devices.pass_ended(session, Err(AllodiaSyncFailure::NeedsReauth));
    assert_eq!(
        devices.view(|_| FREE).expect("a failure is said").failure,
        Some(AllodiaSyncFailure::NeedsReauth)
    );

    let session = devices.want_pass().expect("nothing runs");
    assert_eq!(devices.view(|_| FREE).expect("it runs").failure, None);
    devices.pass_ended(session, Ok(report()));
    assert_eq!(devices.view(|_| FREE).expect("found").failure, None);
}

#[test]
fn a_sharing_change_that_took_answers_its_questions_and_one_that_failed_is_said_on_them() {
    let devices = Devices::default();
    let session = devices.want_pass().expect("nothing runs yet");
    devices.pass_ended(session, Ok(report()));

    devices.sharing_changed("moved", Err(AllodiaSyncFailure::Unavailable));
    let card = devices.view(|_| FREE).expect("found");
    assert_eq!(
        card.questions[0].failed,
        Some(AllodiaSyncFailure::Unavailable)
    );
    assert_eq!(
        devices.sharing_failed("moved"),
        Some(AllodiaSyncFailure::Unavailable)
    );

    devices.sharing_changed("moved", Ok(()));
    let card = devices.view(|_| FREE).expect("found");
    assert_eq!(card.questions.len(), 1);
    assert_eq!(card.questions[0].account_id, "gone");
    assert_eq!(devices.sharing_failed("moved"), None);
}

#[test]
fn a_removed_account_leaves_the_card() {
    let devices = Devices::default();
    let session = devices.want_pass().expect("nothing runs yet");
    devices.pass_ended(session, Ok(report()));
    devices.forget_account("gone");
    let card = devices.view(|_| FREE).expect("found");
    assert!(card.questions.iter().all(|q| q.account_id != "gone"));
    assert!(!card.nothing_found, "the offer still stands");
}

#[test]
fn each_keep_is_drawn_as_the_core_says() {
    let devices = Devices::default();
    let session = devices.want_pass().expect("nothing runs yet");
    devices.pass_ended(session, Ok(report()));
    let card = devices
        .view(|account| ControlState {
            enabled: false,
            running: account == "moved",
        })
        .expect("found");
    assert!(card.questions[0].keep.running);
    assert!(!card.questions[1].keep.running);
}
