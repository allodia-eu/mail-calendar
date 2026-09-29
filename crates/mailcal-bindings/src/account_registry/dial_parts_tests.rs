//! Which capability's failure is the account's.

use mailcal_account::AccountError;

use super::{Part, assemble};
use crate::account_registry::dial::ConnectFailure;

fn unreachable(what: &str) -> ConnectFailure {
    ConnectFailure::from(AccountError::Graph(format!("{what} unreachable")))
}

fn signin_expired() -> ConnectFailure {
    ConnectFailure::from(AccountError::SigninRejected("invalid_grant".to_owned()))
}

#[test]
fn a_mail_failure_is_the_accounts_failure_whatever_the_calendar_did() {
    // How every account behaved before it could be used for less than everything, and how an
    // account used for mail still behaves: its mailbox is the account.
    let result = assemble::<u8, u8, u8>(
        Part::Failed(unreachable("mail")),
        Part::Bound(vec![1]),
        Part::Bound(vec![2]),
    );
    assert!(result.unwrap_err().to_string().contains("mail"));
}

#[test]
fn a_calendar_failure_beside_working_mail_empties_only_the_calendar() {
    let assembled = assemble::<u8, u8, u8>(
        Part::Bound(vec![1]),
        Part::Failed(unreachable("calendar")),
        Part::Bound(vec![2]),
    )
    .unwrap();
    assert_eq!(assembled.mail, [1]);
    assert!(assembled.calendar.is_empty());
    assert_eq!(assembled.contacts, [2]);
    assert!(assembled.calendar_failure.is_some());
}

#[test]
fn an_account_without_mail_is_connected_when_anything_it_is_used_for_connected() {
    let assembled = assemble::<u8, u8, u8>(
        Part::Off,
        Part::Failed(unreachable("calendar")),
        Part::Bound(vec![2]),
    )
    .unwrap();
    assert!(assembled.mail.is_empty());
    assert_eq!(assembled.contacts, [2]);
    assert!(assembled.calendar_failure.is_some());
}

#[test]
fn an_account_without_mail_reports_why_nothing_connected() {
    // A calendar-only account whose grant is dead must be able to say "sign in again", which
    // only its calendar can tell it.
    let failure = assemble::<u8, u8, u8>(Part::Off, Part::Failed(signin_expired()), Part::Off)
        .expect_err("nothing connected");
    assert!(failure.signin_expired());

    // The calendar's reason comes first when both failed.
    let failure = assemble::<u8, u8, u8>(
        Part::Off,
        Part::Failed(unreachable("calendar")),
        Part::Failed(unreachable("contacts")),
    )
    .expect_err("nothing connected");
    assert!(failure.to_string().contains("calendar"));
}

#[test]
fn a_capability_the_account_is_not_used_for_binds_nothing() {
    let assembled = assemble::<u8, u8, u8>(Part::Bound(vec![1]), Part::Off, Part::Off).unwrap();
    assert_eq!(assembled.mail, [1]);
    assert!(assembled.calendar.is_empty() && assembled.contacts.is_empty());
    assert!(assembled.calendar_failure.is_none());
}
