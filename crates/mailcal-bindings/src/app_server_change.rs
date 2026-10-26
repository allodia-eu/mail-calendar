//! A calendar or contacts server found on another site, waiting for the person to allow it
//! (`docs/account-autodetect.md` rule 14).
//!
//! Looking again for a server that is gone may find one on a site the account never used. That
//! is how a provider moving its servers looks, and also how a changed or tampered DNS answer
//! looks, so the use is paused rather than followed, and nothing is sent to the new server until
//! the person allows it.

use engine_api::AccountId;
use mailcal_account::Capability;

use crate::{AccountEndpoints, MailcalApp, MailcalError};

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

/// What allowing a server change came to, when nothing failed.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ServerChangeAnswer {
    /// Nothing waits on the person any more: the account moved onto the new servers, now or by
    /// an answer given elsewhere, or the use connected again by itself.
    Settled,
    /// A lookup replaced the question shown before it was allowed. Nothing moved; `change` is
    /// the question to show in its place.
    Replaced {
        /// The server change the account waits on now.
        change: ServerChange,
    },
}

#[uniffi::export]
impl MailcalApp {
    /// The server change `account_id`'s calendar or contacts is paused for, for the dialog that
    /// asks the person; `None` when it waits on none. Reached from a use whose problem is
    /// [`UseProblem::ServerChanged`](crate::UseProblem::ServerChanged).
    #[must_use]
    pub fn pending_server_change(&self, account_id: String) -> Option<ServerChange> {
        let id = AccountId::try_from(account_id.as_str()).ok()?;
        self.app.server_change(&id).as_ref().map(ServerChange::from)
    }

    /// Allows the servers found on another site, if they are still the ones `shown`: moves the
    /// account onto them once they have connected ([`MailcalApp::update_account_endpoints`]),
    /// and lifts the pause. A lookup may have replaced the question while it was on screen; then
    /// nothing moves, and the answer is the question to ask instead, because an allow must never
    /// send the credential to a site the person was not shown. Declining needs no call: the use
    /// stays paused, and the account's page offers the question again.
    ///
    /// **Blocking** (provider connect); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an id that is not an account's or an account with no
    /// servers to change,
    /// [`MailcalError::UseNotConnected`] when a use it moves does not connect on its new server,
    /// and what [`MailcalApp::update_account_endpoints`] returns when the account does not; the
    /// account stays paused as it was.
    pub fn accept_server_change(
        &self,
        account_id: String,
        shown: ServerChange,
    ) -> Result<ServerChangeAnswer, MailcalError> {
        let id = AccountId::try_from(account_id.as_str())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        // Allowed from another surface meanwhile, or the use connected again by itself: either
        // way the question is over.
        let Some(change) = self.app.server_change(&id) else {
            return Ok(ServerChangeAnswer::Settled);
        };
        let pending = ServerChange::from(&change);
        if pending != shown {
            return Ok(ServerChangeAnswer::Replaced { change: pending });
        }
        let stored = self
            .accounts_snapshot()
            .accounts
            .into_iter()
            .find(|entry| entry.id == account_id)
            .and_then(|entry| entry.endpoints)
            .ok_or_else(|| {
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
        self.replace_endpoints(account_id, endpoints, &moved)?;
        self.app.forget_server_change(&id, &change);
        Ok(ServerChangeAnswer::Settled)
    }
}

#[cfg(test)]
#[path = "app_server_change_tests.rs"]
mod tests;
