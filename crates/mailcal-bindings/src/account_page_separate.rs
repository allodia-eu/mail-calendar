//! A mailbox's calendar and contacts on a server its sign-in does not reach (`docs/accounts.md`
//! rule 18): which of them need a sign-in of their own and where, what that server said it takes
//! besides a password, and what the last Connect or Skip came to. Setup's step after the add and
//! the account's page draw the same section; no client keeps a copy.

use std::{collections::HashMap, sync::Mutex};

use mailcal_account::Capability;

use crate::{
    AccountCapability, AccountChangeRefusal, AccountSeparateServer, AccountSeparateSignIn,
    ControlState,
    account_answers::Answers,
    account_work::{Hold, PageWork},
    accounts_view::{AccountFacts, host_of},
};

/// The uses of a mailbox that need a sign-in of their own, and the server they are at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SeparateNeed {
    /// Calendar before contacts.
    pub(crate) uses: Vec<AccountCapability>,
    pub(crate) host: String,
    /// The login the mailbox holds there, else its address.
    pub(crate) username: String,
}

/// What `account` needs a sign-in of its own for: the uses it is used for whose server needs one
/// or refused the account's, given what stopped each from connecting (`problems`).
pub(crate) fn need(
    account: &AccountFacts,
    problems: mailcal_app::UseProblems,
) -> Option<SeparateNeed> {
    let needs = |capability| {
        let problem = match capability {
            Capability::Calendar => problems.calendar,
            Capability::Contacts => problems.contacts,
            _ => None,
        };
        account.chosen.contains(capability)
            && matches!(
                problem,
                Some(
                    mailcal_app::UseProblem::NeedsOwnSignIn
                        | mailcal_app::UseProblem::SignInRefused
                )
            )
    };
    let servers: Vec<_> = account
        .dav_servers
        .iter()
        .filter(|server| needs(server.capability))
        .collect();
    let first = servers.first()?;
    let login = first.username.trim();
    Some(SeparateNeed {
        uses: servers
            .iter()
            .map(|server| server.capability.into())
            .collect(),
        host: host_of(&first.base_url)?,
        username: if login.is_empty() {
            account.address.clone()
        } else {
            login.to_owned()
        },
    })
}

/// What a separate server said it takes besides a password, for the uses it was asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Offer {
    pub(crate) uses: Vec<AccountCapability>,
    pub(crate) sign_in: Option<AccountSeparateSignIn>,
}

/// A Connect or Skip that left the uses where they were.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
    /// The login it was tried with, which the form is drawn with again.
    pub(crate) username: String,
    pub(crate) refusal: AccountChangeRefusal,
}

/// Every account's separate server as its page last learned it.
#[derive(Debug, Default)]
pub(crate) struct SeparateState {
    pub(crate) offers: Answers<Offer>,
    failures: Mutex<HashMap<String, Failure>>,
}

impl SeparateState {
    /// What `account`'s last Connect or Skip came to: `failed` when it left the uses where they
    /// were, `None` once one worked or nothing needs one.
    pub(crate) fn tried(&self, account: &str, failed: Option<Failure>) {
        let mut failures = self.failures.lock().expect("separate mutex poisoned");
        match failed {
            Some(failure) => failures.insert(account.to_owned(), failure),
            None => failures.remove(account),
        };
    }

    /// `account`'s section for `need`, held as `hold` says.
    pub(crate) fn section(
        &self,
        account: &str,
        need: SeparateNeed,
        hold: &Hold,
    ) -> AccountSeparateServer {
        let failure = self
            .failures
            .lock()
            .expect("separate mutex poisoned")
            .get(account)
            .cloned();
        section(
            need,
            self.offers.get(account).as_ref(),
            failure.as_ref(),
            hold,
        )
    }

    /// Drops what is kept for `account`, which is gone.
    pub(crate) fn forget(&self, account: &str) {
        self.offers.forget(account);
        self.tried(account, None);
    }
}

/// The section for `need`, from what its server offered (an answer about other uses is no
/// answer) and the last attempt's `failure`, held as `hold` says.
pub(crate) fn section(
    need: SeparateNeed,
    offer: Option<&Offer>,
    failure: Option<&Failure>,
    hold: &Hold,
) -> AccountSeparateServer {
    let connecting = hold.going(|work| match work {
        PageWork::SeparateServer { username } => Some(username.clone()),
        _ => None,
    });
    let stopping = hold.own.contains(&PageWork::StopSeparate);
    // An attempt running says nothing of the one before it.
    let failure = failure.filter(|_| connecting.is_none() && !stopping);
    AccountSeparateServer {
        username: connecting
            .clone()
            .or_else(|| failure.map(|failure| failure.username.clone()))
            .unwrap_or(need.username),
        sign_in: offer
            .filter(|offer| offer.uses == need.uses)
            .and_then(|offer| offer.sign_in),
        connect: ControlState {
            enabled: !hold.busy,
            running: connecting.is_some(),
        },
        stop_using: ControlState {
            enabled: !hold.busy,
            running: stopping,
        },
        failed: failure.map(|failure| failure.refusal.clone()),
        uses: need.uses,
        host: need.host,
    }
}

#[cfg(test)]
#[path = "account_page_separate_tests.rs"]
mod tests;
