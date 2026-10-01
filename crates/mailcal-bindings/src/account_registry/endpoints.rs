//! A standards account's edited servers, checked against the registry before anything dials them.

use mailcal_account::{Capabilities, EndpointEdit};

use super::AccountRegistry;
use crate::{ConnectedAccount, MailcalError};

impl AccountRegistry {
    /// `id`'s config with `edit` applied, serialized for a dial and the host's store, and the uses
    /// whose server moved to another host. Nothing in the registry changes.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an unknown account, one that signs in through its
    /// provider, an edit the account cannot take, and one that would make it the same account as
    /// another already set up.
    pub(crate) fn edited_endpoints(
        &self,
        id: &str,
        edit: &EndpointEdit,
    ) -> Result<(String, Capabilities), MailcalError> {
        let refused = |reason: String| MailcalError::Config(reason);
        let entries = self
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        let config = match entries.get(id) {
            Some(ConnectedAccount::Imap { config, .. }) if !config.is_oauth() => config,
            Some(_) => {
                return Err(refused(
                    "this account's servers are its provider's; sign it in again instead"
                        .to_owned(),
                ));
            }
            None => return Err(refused("no such account".to_owned())),
        };
        let edited = config
            .with_endpoints(edit)
            .map_err(|err| refused(err.to_string()))?;
        // Another account already keyed by the id these servers would derive is the same
        // mailbox set up twice.
        let duplicate = edited
            .config
            .derived_account_id()
            .is_ok_and(|derived| derived.as_str() != id && entries.contains_key(derived.as_str()));
        if duplicate {
            return Err(refused("that account is already set up".to_owned()));
        }
        let config_toml = edited
            .config
            .to_toml()
            .map_err(|err| refused(err.to_string()))?;
        Ok((config_toml, edited.moved))
    }
}
