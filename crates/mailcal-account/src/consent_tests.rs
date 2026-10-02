use std::collections::BTreeSet;

use mailcal_oauth::{
    GOOGLE_SCOPES, MICROSOFT_GRAPH_SCOPES,
    scopes::{GOOGLE, MICROSOFT},
};

use super::{requested_scopes, withheld};
use crate::{Capabilities, Capability};

fn set(capabilities: &[Capability]) -> Capabilities {
    capabilities.iter().copied().collect()
}

fn as_set(scopes: &[String]) -> BTreeSet<&str> {
    scopes.iter().map(String::as_str).collect()
}

#[test]
fn an_account_used_for_everything_asks_for_what_every_account_asked_for_before() {
    let all = set(&Capability::ALL);
    assert_eq!(
        as_set(&requested_scopes(&MICROSOFT, &all)),
        MICROSOFT_GRAPH_SCOPES.iter().copied().collect()
    );
    assert_eq!(
        as_set(&requested_scopes(&GOOGLE, &all)),
        GOOGLE_SCOPES.iter().copied().collect()
    );
}

#[test]
fn a_calendar_alone_asks_for_no_mail_and_no_contacts() {
    let scopes = requested_scopes(&MICROSOFT, &set(&[Capability::Calendar]));
    assert_eq!(
        as_set(&scopes),
        BTreeSet::from([
            "offline_access",
            "https://graph.microsoft.com/User.Read",
            "https://graph.microsoft.com/Calendars.ReadWrite",
        ])
    );
}

#[test]
fn every_google_sign_in_asks_for_the_address_whatever_it_is_used_for() {
    let scopes = requested_scopes(&GOOGLE, &set(&[Capability::Calendar]));
    assert_eq!(
        as_set(&scopes),
        BTreeSet::from([
            "https://www.googleapis.com/auth/calendar",
            "https://www.googleapis.com/auth/userinfo.email",
        ])
    );
    // A mail account asks too, so that a consent screen with Gmail unticked still names it.
    let with_mail = requested_scopes(&GOOGLE, &set(&[Capability::Mail]));
    assert!(
        with_mail
            .iter()
            .any(|scope| scope.ends_with("auth/userinfo.email"))
    );
}

#[test]
fn colleagues_are_asked_for_only_beside_contacts() {
    let alone = requested_scopes(
        &MICROSOFT,
        &set(&[Capability::Mail, Capability::Colleagues]),
    );
    assert!(
        !alone
            .iter()
            .any(|scope| scope.ends_with("User.ReadBasic.All"))
    );
    let beside = requested_scopes(
        &MICROSOFT,
        &set(&[Capability::Contacts, Capability::Colleagues]),
    );
    assert!(
        beside
            .iter()
            .any(|scope| scope.ends_with("User.ReadBasic.All"))
    );
}

#[test]
fn a_grant_nobody_recorded_withholds_nothing() {
    assert!(withheld(&GOOGLE, &set(&Capability::ALL), None).is_empty());
}

#[test]
fn a_chosen_use_whose_scope_was_refused_is_withheld() {
    // A person who unticked the calendar on Google's consent screen.
    let granted = vec![
        "https://mail.google.com/".to_owned(),
        "https://www.googleapis.com/auth/contacts".to_owned(),
    ];
    let chosen = set(&[Capability::Mail, Capability::Calendar, Capability::Contacts]);
    assert_eq!(
        withheld(&GOOGLE, &chosen, Some(&granted)),
        set(&[Capability::Calendar])
    );
}

#[test]
fn a_missing_scope_that_serves_one_feature_withholds_nothing() {
    // A grant from before `Mail.Send` was asked for still reads mail; sending is refused when it
    // is reached, which is where the reconnect-to-send prompt is raised.
    let granted = vec![
        "Mail.ReadWrite".to_owned(),
        "Calendars.ReadWrite".to_owned(),
    ];
    let chosen = set(&[Capability::Mail, Capability::Calendar]);
    assert!(withheld(&MICROSOFT, &chosen, Some(&granted)).is_empty());
}

#[test]
fn a_use_nobody_chose_is_never_withheld() {
    let granted = vec!["https://mail.google.com/".to_owned()];
    assert!(withheld(&GOOGLE, &set(&[Capability::Mail]), Some(&granted)).is_empty());
}
