//! Telling apart accounts a person would read by the same address.

use crate::{AccountDistinction, accounts_view::AccountFacts};

/// What tells each account apart from the others with its address, in the order `facts` lists
/// them: its kind where no other of them shares it, else its server's host. `None` for an
/// account no other has the address of, or for one with no host to name.
pub(crate) fn distinctions(facts: &[AccountFacts]) -> Vec<Option<AccountDistinction>> {
    facts
        .iter()
        .enumerate()
        .map(|(index, account)| {
            let namesakes: Vec<&AccountFacts> = facts
                .iter()
                .enumerate()
                .filter(|&(other, theirs)| {
                    other != index && engine_api::addresses_match(&theirs.address, &account.address)
                })
                .map(|(_, theirs)| theirs)
                .collect();
            if namesakes.is_empty() {
                None
            } else if namesakes.iter().all(|theirs| theirs.kind != account.kind) {
                Some(AccountDistinction::Kind { kind: account.kind })
            } else {
                account
                    .server
                    .clone()
                    .map(|host| AccountDistinction::Server { host })
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "accounts_distinction_tests.rs"]
mod tests;
