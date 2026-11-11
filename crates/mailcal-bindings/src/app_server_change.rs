//! A calendar or contacts server found on another site, waiting for the person to allow it
//! (`docs/account-autodetect.md` rule 14).
//!
//! Looking again for a server that is gone may find one on a site the account never used. That
//! is how a provider moving its servers looks, and also how a changed or tampered DNS answer
//! looks, so the use is paused rather than followed, and nothing is sent to the new server until
//! the person allows it. Allowing is a change on the account's page (`docs/waiting.md`), whichever
//! surface asked the question.

use engine_api::AccountId;
use mailcal_account::Capability;

use crate::{AccountEndpoints, ControlState, MailcalApp, MailcalError, account_work::PageWork};

/// Servers found on another site for an account's calendar or contacts.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ServerChange {
    /// The site the calendar or contacts connected to before, such as `icloud.com`: the
    /// registrable domain, which is what a person can recognise.
    pub previous_site: String,
    /// The site the new server is on.
    pub proposed_site: String,
    /// Whether the calendar moved.
    pub calendar: bool,
    /// Whether the contacts moved.
    pub contacts: bool,
}

impl From<&mailcal_app::ServerChange> for ServerChange {
    fn from(change: &mailcal_app::ServerChange) -> Self {
        Self {
            previous_site: change.previous_site.clone(),
            proposed_site: change.proposed_site.clone(),
            calendar: change.calendar_url.is_some(),
            contacts: change.contacts_url.is_some(),
        }
    }
}

/// The question that allows an account's new servers, as a client draws it: from the account's
/// page, the calendar or the contacts list alike. Open while it is `Some`; once the account waits
/// on none, it is over.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountServerChange {
    /// The account, which the question names by its address, told apart from another account
    /// with that address as Settings tells it (`docs/accounts.md` rule 8).
    pub account: crate::LinkedAccount,
    /// What the question asks about: the change being allowed while that runs, else the one the
    /// account waits on now, which a lookup may replace while it is shown.
    pub change: ServerChange,
    /// "Allow", sent as [`AccountPageIntent::AllowServerChange`](crate::AccountPageIntent) with
    /// `change`. While it runs, "Not now" waits for it too: closing would not stop it.
    pub allow: ControlState,
    /// The last allow of this very change did not connect, so nothing changed.
    pub failed: bool,
}

/// What allowing a server change came to, when nothing failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ServerChangeAnswer {
    /// Nothing waits on the person any more: the account moved onto the new servers, now or by
    /// an answer given elsewhere, or the use connected again by itself.
    Settled,
    /// A lookup replaced the question shown before it was allowed. Nothing moved; the question
    /// asks the new change from now.
    Replaced,
}

#[uniffi::export]
impl MailcalApp {
    /// The question `account_id`'s calendar or contacts is paused for; `None` when it waits on
    /// none. Reached from a use whose problem is
    /// [`UseProblem::ServerChanged`](crate::UseProblem::ServerChanged). Read again after every
    /// `Surface::AccountPage` signal, which an allow raises as it starts and ends, and every
    /// `Surface::Notices` one, which a lookup replacing the question raises, while it is open.
    #[must_use]
    pub fn account_server_change(&self, account_id: String) -> Option<AccountServerChange> {
        let id = AccountId::try_from(account_id.as_str()).ok()?;
        let (hold, _) = self.account_work.of(&account_id);
        let allowing = hold.going(|work| match work {
            PageWork::ServerChange(shown) => Some(shown.clone()),
            _ => None,
        });
        let change = allowing
            .clone()
            .or_else(|| self.app.server_change(&id).as_ref().map(ServerChange::from))?;
        let failed =
            allowing.is_none() && self.account_work.server_change_failed(&account_id, &change);
        Some(AccountServerChange {
            account: self.named_account(&account_id)?,
            change,
            allow: ControlState {
                enabled: !hold.busy,
                running: allowing.is_some(),
            },
            failed,
        })
    }
}

impl MailcalApp {
    /// `account_id` as Settings names it, from the registry alone.
    fn named_account(&self, account_id: &str) -> Option<crate::LinkedAccount> {
        let facts = self.registry.facts();
        let told = crate::accounts_distinction::distinctions(&facts);
        facts
            .iter()
            .zip(told)
            .find(|(account, _)| account.id == account_id)
            .map(|(account, distinction)| crate::LinkedAccount {
                id: account.id.clone(),
                address: account.address.clone(),
                distinction,
            })
    }

    /// Allows `shown` for `account_id`, as a change on its page: what it came to is said by the
    /// question itself, so it leaves no notice on the page.
    pub(crate) fn allow_server_change(&self, account_id: &str, shown: &ServerChange) {
        match self.accept_server_change(account_id, shown) {
            Ok(ServerChangeAnswer::Settled) => {
                self.account_work.server_change_tried(account_id, None)
            }
            Ok(ServerChangeAnswer::Replaced) => {
                log::info!("accounts: the server change was replaced while it was shown");
            }
            Err(error) => {
                log::warn!(
                    "accounts: a new calendar or address book server did not connect: {error}"
                );
                self.account_work
                    .server_change_tried(account_id, Some(shown.clone()));
            }
        }
    }

    /// Allows the servers found on another site, if they are still the ones `shown`: moves the
    /// account onto them once they have connected ([`MailcalApp::update_account_endpoints`]),
    /// and lifts the pause. A lookup may have replaced the question while it was on screen; then
    /// nothing moves, because an allow must never send the credential to a site the person was
    /// not shown. Declining needs no call: the use stays paused, and the question is offered
    /// again.
    ///
    /// **Blocking** (provider connect).
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an id that is not an account's or an account with no
    /// servers to change,
    /// [`MailcalError::UseNotConnected`] when a use it moves does not connect on its new server,
    /// and what [`MailcalApp::update_account_endpoints`] returns when the account does not; the
    /// account stays paused as it was.
    pub(crate) fn accept_server_change(
        &self,
        account_id: &str,
        shown: &ServerChange,
    ) -> Result<ServerChangeAnswer, MailcalError> {
        let id =
            AccountId::try_from(account_id).map_err(|err| MailcalError::Config(err.to_string()))?;
        // Allowed from another surface meanwhile, or the use connected again by itself: either
        // way the question is over.
        let Some(change) = self.app.server_change(&id) else {
            return Ok(ServerChangeAnswer::Settled);
        };
        if ServerChange::from(&change) != *shown {
            return Ok(ServerChangeAnswer::Replaced);
        }
        let stored = self.registry.endpoints(account_id).ok_or_else(|| {
            MailcalError::Config("the account has no servers to change".to_owned())
        })?;
        let endpoints = AccountEndpoints {
            caldav_url: change.calendar_url.clone().or(stored.caldav_url.clone()),
            carddav_url: change.contacts_url.clone().or(stored.carddav_url.clone()),
            password: None,
            ..stored
        };
        // A mailbox that connects is not enough: the uses that move have to connect on the new
        // servers too, or the account stays paused where it was.
        let moved: Vec<_> = [
            (change.calendar_url.is_some(), Capability::Calendar),
            (change.contacts_url.is_some(), Capability::Contacts),
        ]
        .into_iter()
        .filter_map(|(moved, capability)| moved.then_some(capability))
        .collect();
        self.replace_endpoints(account_id.to_owned(), endpoints, &moved)?;
        self.app.forget_server_change(&id, &change);
        Ok(ServerChangeAnswer::Settled)
    }
}

#[cfg(test)]
#[path = "app_server_change_tests.rs"]
mod tests;
