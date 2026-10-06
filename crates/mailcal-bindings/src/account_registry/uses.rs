//! Switching one of an account's uses on or off in the registry.

use mailcal_account::{AccountShape, Capabilities, Capability};

use super::AccountRegistry;
use crate::{BoxedAccount, ConnectedAccount, MailcalError};

/// What [`AccountRegistry::set_use`] did.
pub(crate) enum UseChange {
    /// The account already was, or was not, used for it.
    Unchanged,
    /// The entry now holds the new choice, with its id pinned.
    Changed {
        /// The entry's config, re-serialized for the host's store.
        config_toml: String,
        /// What the entry stored before, for [`AccountRegistry::restore_uses`] when the store
        /// refuses the new config.
        previous: AccountShape,
    },
    /// The provider's grant does not allow it; nothing changed.
    NeedsConsent,
    /// The account has no server for it; nothing changed.
    NeedsEndpoint,
}

impl AccountRegistry {
    /// Switches `capability` on or off for `id`. Switching contacts off switches colleagues off
    /// with it, since they are only read beside contacts.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an unknown account, a use its kind cannot offer,
    /// colleagues without contacts, or a change that would leave it used for none of mail,
    /// calendar and contacts: such an account is removed, never emptied.
    pub(crate) fn set_use(
        &self,
        id: &str,
        capability: Capability,
        on: bool,
    ) -> Result<UseChange, MailcalError> {
        let refused = |reason: &str| Err(MailcalError::Config(reason.to_owned()));
        let mut entries = self
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        let Some(entry) = entries.get_mut(id) else {
            return refused("no such account");
        };
        let chosen = entry.capabilities();
        if chosen.contains(capability) == on {
            return Ok(UseChange::Unchanged);
        }
        let next: Capabilities = chosen
            .iter()
            .filter(|&kept| {
                on || (kept != capability
                    && !(capability == Capability::Contacts && kept == Capability::Colleagues))
            })
            .chain(on.then_some(capability))
            .collect();
        if [Capability::Mail, Capability::Calendar, Capability::Contacts]
            .into_iter()
            .all(|usable| !next.contains(usable))
        {
            return refused("an account is used for at least one of mail, calendar and contacts");
        }
        if capability == Capability::Colleagues && on && !next.contains(Capability::Contacts) {
            return refused("colleagues are only read beside contacts");
        }
        if on {
            match opens(entry, &next, capability) {
                Opens::Yes => {}
                Opens::NotOffered => return refused("this kind of account does not offer that"),
                Opens::NeedsConsent => return Ok(UseChange::NeedsConsent),
                Opens::NeedsEndpoint => return Ok(UseChange::NeedsEndpoint),
            }
        }
        let account_id = entry.account_id();
        let shape = entry.shape_mut();
        let previous = shape.clone();
        shape.id = shape.id.take().or(account_id);
        shape.capabilities = Some(next);
        match entry.to_toml() {
            Ok(config_toml) => Ok(UseChange::Changed {
                config_toml,
                previous,
            }),
            Err(err) => {
                *entry.shape_mut() = previous;
                Err(MailcalError::Config(err.to_string()))
            }
        }
    }

    /// Puts back the uses and id `id` stored before a change the host's store refused. Its links
    /// are left as they are: one set since then is not this change's to undo.
    pub(crate) fn restore_uses(&self, id: &str, previous: AccountShape) {
        if let Some(entry) = self
            .entries
            .lock()
            .expect("account registry mutex poisoned")
            .get_mut(id)
        {
            let shape = entry.shape_mut();
            shape.id = previous.id;
            shape.capabilities = previous.capabilities;
        }
    }

    /// The provider-less account the app lists for `id` until its next dial lands, or `None` if it
    /// is not registered.
    pub(crate) fn placeholder(&self, id: &engine_api::AccountId) -> Option<BoxedAccount> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id.as_str())
            .map(|entry| crate::boot::placeholder(id.clone(), entry))
    }
}

/// Whether switching `capability` on, making the choice `next`, can open it.
enum Opens {
    Yes,
    NotOffered,
    NeedsConsent,
    NeedsEndpoint,
}

fn opens(entry: &ConnectedAccount, next: &Capabilities, capability: Capability) -> Opens {
    let granted = |withheld: Capabilities| {
        if withheld.contains(capability) {
            Opens::NeedsConsent
        } else {
            Opens::Yes
        }
    };
    match entry {
        ConnectedAccount::Microsoft { config, .. } if !config.offered().contains(capability) => {
            Opens::NotOffered
        }
        ConnectedAccount::Microsoft { config, .. } => granted(mailcal_account::withheld(
            &mailcal_oauth::scopes::MICROSOFT,
            next,
            config.granted_scopes.as_deref(),
        )),
        ConnectedAccount::Google { config, .. } => granted(mailcal_account::withheld(
            &mailcal_oauth::scopes::GOOGLE,
            next,
            config.granted_scopes.as_deref(),
        )),
        _ if capability == Capability::Colleagues => Opens::NotOffered,
        ConnectedAccount::Imap { config, .. } => {
            let has_server = match capability {
                Capability::Mail => config.imap.is_some(),
                Capability::Calendar => config.caldav.is_some(),
                Capability::Contacts => config.carddav.is_some() || config.caldav.is_some(),
                Capability::Colleagues => false,
            };
            if has_server {
                Opens::Yes
            } else {
                Opens::NeedsEndpoint
            }
        }
        // The session says which domains the account has; the dial binds what it offers.
        ConnectedAccount::Jmap { .. } => Opens::Yes,
    }
}
