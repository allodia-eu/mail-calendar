//! Switching one of an account's uses on or off in the registry.

use mailcal_account::{AccountShape, Capabilities, Capability};

use super::AccountRegistry;
use crate::{AccountUseBlock, BoxedAccount, ConnectedAccount, MailcalError};

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
    /// The provider has not granted it; nothing changed.
    NeedsConsent,
    /// The account has no server for it; nothing changed.
    NeedsEndpoint,
}

/// What switching a use on or off comes to, decided once: [`AccountRegistry::set_use`] acts on
/// it, and an account's page reads it before the switch is pressed (`docs/waiting.md` rule 6), so
/// the check that refuses a change and the page that predicts it cannot disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Toggle {
    /// Already so: nothing to do.
    Unchanged,
    /// The core makes the change; `next` is what the account is used for after it.
    Applies { next: Capabilities },
    /// The provider has not granted it: switching it on asks the provider for it.
    AsksProvider,
    /// It cannot be made, for this reason.
    Refused(AccountUseBlock),
}

/// What switching `capability` on or off comes to for `entry`, whose provider refused the uses
/// in `refused` when they were opened although its stored grant allows them. Switching contacts
/// off takes colleagues with it, since they are only read beside contacts, and switching mail off
/// takes shared mailboxes with it.
pub(crate) fn toggle(
    entry: &ConnectedAccount,
    capability: Capability,
    on: bool,
    refused: &Capabilities,
) -> Toggle {
    let lacks = entry.lacks();
    if on && lacks.contains(capability) {
        return Toggle::Refused(AccountUseBlock::NotOffered);
    }
    let chosen = entry.capabilities();
    if chosen.contains(capability) == on {
        // A use chosen and not granted is not working, so switching it on asks again.
        let withheld = on
            && (refused.contains(capability)
                || matches!(opens(entry, &chosen, capability), Opens::NeedsConsent));
        return if withheld {
            Toggle::AsksProvider
        } else {
            Toggle::Unchanged
        };
    }
    let next: Capabilities = chosen
        .iter()
        .filter(|&kept| on || (kept != capability && !depends_on(kept, capability)))
        .chain(on.then_some(capability))
        .collect();
    // An account used for none of these is not one: it is removed, never emptied.
    if [Capability::Mail, Capability::Calendar, Capability::Contacts]
        .into_iter()
        .all(|usable| !next.contains(usable) || lacks.contains(usable))
    {
        return Toggle::Refused(AccountUseBlock::LastUse);
    }
    if on && capability == Capability::Colleagues && !next.contains(Capability::Contacts) {
        return Toggle::Refused(AccountUseBlock::NeedsContacts);
    }
    if on && capability == Capability::SharedMailboxes && !next.contains(Capability::Mail) {
        return Toggle::Refused(AccountUseBlock::NeedsMail);
    }
    if on {
        match opens(entry, &next, capability) {
            Opens::Yes => {}
            Opens::NotOffered => return Toggle::Refused(AccountUseBlock::NotOffered),
            Opens::NeedsConsent => return Toggle::AsksProvider,
            Opens::NeedsEndpoint => return Toggle::Refused(AccountUseBlock::NeedsServer),
        }
    }
    Toggle::Applies { next }
}

impl AccountRegistry {
    /// What switching `capability` on or off for `id` comes to, changing nothing; `None` for an
    /// account that is not registered. `refused` is as [`toggle`] takes it.
    pub(crate) fn toggle(
        &self,
        id: &str,
        capability: Capability,
        on: bool,
        refused: &Capabilities,
    ) -> Option<Toggle> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)
            .map(|entry| toggle(entry, capability, on, refused))
    }

    /// Switches `capability` on or off for `id`, as [`toggle`] decides with `refused`.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an unknown account and for every refusal but a missing
    /// server, which is [`UseChange::NeedsEndpoint`]: a use its server does not offer, colleagues
    /// without contacts, shared mailboxes without mail, or a change that would leave the account
    /// used for none of mail, calendar and contacts its server offers.
    pub(crate) fn set_use(
        &self,
        id: &str,
        capability: Capability,
        on: bool,
        refused: &Capabilities,
    ) -> Result<UseChange, MailcalError> {
        let mut entries = self
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        let Some(entry) = entries.get_mut(id) else {
            return Err(MailcalError::Config("no such account".to_owned()));
        };
        let next = match toggle(entry, capability, on, refused) {
            Toggle::Unchanged => return Ok(UseChange::Unchanged),
            Toggle::AsksProvider => return Ok(UseChange::NeedsConsent),
            Toggle::Refused(AccountUseBlock::NeedsServer) => return Ok(UseChange::NeedsEndpoint),
            Toggle::Refused(block) => return Err(MailcalError::Config(block.reason().to_owned())),
            Toggle::Applies { next } => next,
        };
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

/// Whether `sub` is a choice within `capability`, which goes with it.
fn depends_on(sub: Capability, capability: Capability) -> bool {
    matches!(
        (sub, capability),
        (Capability::Colleagues, Capability::Contacts)
            | (Capability::SharedMailboxes, Capability::Mail)
    )
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
        ConnectedAccount::Google { config, .. } if !config.offered().contains(capability) => {
            Opens::NotOffered
        }
        ConnectedAccount::Google { config, .. } => granted(mailcal_account::withheld(
            &mailcal_oauth::scopes::GOOGLE,
            next,
            config.granted_scopes.as_deref(),
        )),
        // Talk meetings use the account's own app password, so a Nextcloud account needs no
        // more than its server's Talk, which the account's lacks have already answered.
        ConnectedAccount::Imap { config, .. }
            if capability == Capability::Meetings && config.nextcloud.is_some() =>
        {
            Opens::Yes
        }
        _ if matches!(
            capability,
            Capability::Colleagues | Capability::SharedMailboxes | Capability::Meetings
        ) =>
        {
            Opens::NotOffered
        }
        ConnectedAccount::Imap { config, .. } => {
            let has_server = match capability {
                Capability::Mail => config.imap.is_some(),
                Capability::Calendar => config.caldav.is_some(),
                Capability::Contacts => config.carddav.is_some() || config.caldav.is_some(),
                Capability::Colleagues | Capability::SharedMailboxes | Capability::Meetings => {
                    false
                }
            };
            if has_server {
                Opens::Yes
            } else {
                Opens::NeedsEndpoint
            }
        }
        // The session says which domains the account has; one it lacks is refused before this.
        ConnectedAccount::Jmap { .. } => Opens::Yes,
    }
}

#[cfg(test)]
impl AccountRegistry {
    /// Where `id`'s dials record whether its Nextcloud server offers Talk.
    pub(crate) fn talk_offer(&self, id: &str) -> Option<crate::talk_offer::TalkOffer> {
        match self
            .entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)
        {
            Some(ConnectedAccount::Imap { talk, .. }) => Some(talk.clone()),
            _ => None,
        }
    }

    /// Where `id`'s dials record its JMAP session, for a test that has no server to read one from.
    pub(crate) fn jmap_session(&self, id: &str) -> Option<crate::jmap_session::JmapSession> {
        match self
            .entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)
        {
            Some(ConnectedAccount::Jmap { session, .. }) => Some(session.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "toggle_tests.rs"]
mod toggle_tests;
