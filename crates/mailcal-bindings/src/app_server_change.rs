//! A calendar or contacts server found on another site, waiting for the person to allow it
//! (`docs/account-autodetect.md` rule 14).
//!
//! Looking again for a server that is gone may find one on a site the account never used. That
//! is how a provider moving its servers looks, and also how a changed or tampered DNS answer
//! looks, so the use is paused rather than followed, and nothing is sent to the new server until
//! the person allows it.

use engine_api::AccountId;

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

    /// Allows the servers found on another site: moves the account onto them once they have
    /// connected ([`MailcalApp::update_account_endpoints`]), and lifts the pause. Declining needs
    /// no call: the use stays paused, and the account's page offers the question again.
    ///
    /// **Blocking** (provider connect); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] when the account waits on no server change, and what
    /// [`MailcalApp::update_account_endpoints`] returns when the new servers do not connect; the
    /// account stays paused as it was.
    pub fn accept_server_change(&self, account_id: String) -> Result<(), MailcalError> {
        let id = AccountId::try_from(account_id.as_str())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let change = self.app.server_change(&id).ok_or_else(|| {
            MailcalError::Config("no server change is waiting for this account".to_owned())
        })?;
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
        self.update_account_endpoints(account_id, endpoints)?;
        self.app.forget_server_change(&id);
        Ok(())
    }
}

#[cfg(test)]
#[path = "app_server_change_tests.rs"]
mod tests;
