use std::collections::BTreeMap;

use mailcal_app::{UseProblem as Problem, UseProblems};

use super::with_outages;
use crate::{ConnectivitySnapshot, UseOutage, UseProblem};

fn snapshot(offline: bool, expired: &[&str]) -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        offline,
        unreachable_accounts: Vec::new(),
        calendar_reauth_accounts: Vec::new(),
        mail_reauth_accounts: Vec::new(),
        signin_expired_accounts: expired.iter().map(|id| (*id).to_owned()).collect(),
        calendar_outages: Vec::new(),
        contacts_outages: Vec::new(),
    }
}

fn outage(account: &str, problem: UseProblem) -> UseOutage {
    UseOutage {
        account_id: account.to_owned(),
        problem,
    }
}

fn problems(entries: &[(&str, Option<Problem>, Option<Problem>)]) -> BTreeMap<String, UseProblems> {
    entries
        .iter()
        .map(|(id, calendar, contacts)| {
            (
                (*id).to_owned(),
                UseProblems {
                    calendar: *calendar,
                    contacts: *contacts,
                },
            )
        })
        .collect()
}

#[test]
fn each_use_that_did_not_connect_is_listed_with_why() {
    let filled = with_outages(
        snapshot(false, &[]),
        &problems(&[
            (
                "a",
                Some(Problem::NeedsOwnSignIn),
                Some(Problem::NeedsOwnSignIn),
            ),
            ("b", None, Some(Problem::SignInRefused)),
            ("c", Some(Problem::Unreachable), None),
        ]),
    );
    assert_eq!(
        filled.calendar_outages,
        vec![
            outage("a", UseProblem::NeedsOwnSignIn),
            outage("c", UseProblem::Unreachable),
        ]
    );
    assert_eq!(
        filled.contacts_outages,
        vec![
            outage("a", UseProblem::NeedsOwnSignIn),
            outage("b", UseProblem::SignInRefused),
        ]
    );
}

/// Offline, every server is unreachable and the offline banner already says why; a use that
/// needs its own sign-in still does.
#[test]
fn offline_leaves_out_what_the_device_is_the_cause_of() {
    let filled = with_outages(
        snapshot(true, &[]),
        &problems(&[(
            "a",
            Some(Problem::Unreachable),
            Some(Problem::NeedsOwnSignIn),
        )]),
    );
    assert!(filled.calendar_outages.is_empty());
    assert_eq!(
        filled.contacts_outages,
        vec![outage("a", UseProblem::NeedsOwnSignIn)]
    );
}

#[test]
fn an_account_whose_sign_in_expired_has_its_own_prompt_instead() {
    let filled = with_outages(
        snapshot(false, &["a"]),
        &problems(&[(
            "a",
            Some(Problem::SignInRefused),
            Some(Problem::SignInRefused),
        )]),
    );
    assert!(filled.calendar_outages.is_empty());
    assert!(filled.contacts_outages.is_empty());
}
