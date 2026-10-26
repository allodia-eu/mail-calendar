//! The calendars and address books the calendar and contacts screens say did not connect
//! (`docs/accounts.md` rule 16), read from what each account's latest dial recorded.

use std::collections::BTreeMap;

use mailcal_app::{UseProblem as Problem, UseProblems};

use crate::{ConnectivitySnapshot, UseOutage};

/// `snapshot` with its calendar and contacts outages filled from `problems`.
///
/// An unreachable use is left out while the device is offline, which the offline banner already
/// says, and so is every use of an account whose sign-in expired, whose own prompt covers it.
pub(crate) fn with_outages(
    mut snapshot: ConnectivitySnapshot,
    problems: &BTreeMap<String, UseProblems>,
) -> ConnectivitySnapshot {
    let outages = |pick: fn(&UseProblems) -> Option<Problem>| {
        problems
            .iter()
            .filter(|(id, _)| !snapshot.signin_expired_accounts.contains(id))
            .filter_map(|(id, uses)| pick(uses).map(|problem| (id, problem)))
            .filter(|(_, problem)| !(snapshot.offline && *problem == Problem::Unreachable))
            .map(|(id, problem)| UseOutage {
                account_id: id.clone(),
                problem: problem.into(),
            })
            .collect::<Vec<_>>()
    };
    let calendar = outages(|uses| uses.calendar);
    let contacts = outages(|uses| uses.contacts);
    snapshot.calendar_outages = calendar;
    snapshot.contacts_outages = contacts;
    snapshot
}

#[cfg(test)]
#[path = "use_outages_tests.rs"]
mod tests;
