//! Which calendar and which mail account the links name, read from the source the binding layer
//! sets.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use engine_api::AccountId;
use mailcal_account::AccountLinks;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

use fakes::{InvitationFake, linked_invitation_app};

fn id(text: &str) -> AccountId {
    AccountId::try_from(text).unwrap()
}

fn app(linked: bool) -> crate::App<InvitationFake> {
    linked_invitation_app(
        InvitationFake::new().with_mail_only(),
        InvitationFake::new(),
        linked,
        &Arc::new(Mutex::new(Vec::new())),
        None,
    )
}

#[test]
fn an_account_without_links_is_its_own_calendar_and_mail() {
    let app = app(false);
    assert_eq!(app.invitation_calendar(&id("acct-a")), id("acct-a"));
    assert_eq!(app.invitation_mail(&id("cal-b")), id("cal-b"));
    assert_eq!(app.invitation_partners(&id("acct-a")), []);
}

#[test]
fn a_link_names_the_calendar_and_the_mail_account_either_way_round() {
    let app = app(true);
    assert_eq!(app.invitation_calendar(&id("acct-a")), id("cal-b"));
    assert_eq!(app.invitation_mail(&id("cal-b")), id("acct-a"));
    assert_eq!(app.invitation_partners(&id("acct-a")), vec![id("cal-b")]);
    assert_eq!(app.invitation_partners(&id("cal-b")), vec![id("acct-a")]);
}

#[test]
fn a_calendar_two_mailboxes_file_into_answers_as_both() {
    let app = app(false);
    app.set_links_source(Arc::new(|| {
        let files_into_b = AccountLinks {
            calendar: Some(id("cal-b")),
            ..AccountLinks::default()
        };
        BTreeMap::from([
            ("acct-a".to_owned(), files_into_b.clone()),
            ("acct-c".to_owned(), files_into_b),
            (
                "cal-b".to_owned(),
                AccountLinks {
                    mail: Some(id("acct-a")),
                    ..AccountLinks::default()
                },
            ),
        ])
    }));
    assert_eq!(
        app.invitation_partners(&id("cal-b")),
        vec![id("acct-a"), id("acct-c")]
    );
}

#[test]
fn a_link_cleared_at_the_source_stops_counting_at_once() {
    let app = app(true);
    app.set_links_source(Arc::new(BTreeMap::new));
    assert_eq!(app.invitation_calendar(&id("acct-a")), id("acct-a"));
}
