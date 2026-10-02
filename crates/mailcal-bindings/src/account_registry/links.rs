//! What the registry tells Settings → Accounts, and the links it clears when an account goes.

use engine_api::AccountId;

use super::AccountRegistry;
use crate::{
    LinkSlot, MailcalError,
    accounts_view::{AccountFacts, candidates, valid_links},
};

/// Each account a change wrote to, with its config re-serialized for the host's store.
type Changed = Vec<(String, Result<String, String>)>;

impl AccountRegistry {
    /// Every registered account's facts, in no particular order.
    pub(crate) fn facts(&self) -> Vec<AccountFacts> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .iter()
            .map(|(id, entry)| entry.facts(id))
            .collect()
    }

    /// `id`'s config as the registry holds it now, serialized for the host's store, or `None` if it
    /// is not registered.
    pub(crate) fn config_toml(&self, id: &str) -> Option<Result<String, String>> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)
            .map(|entry| entry.to_toml().map_err(|err| err.to_string()))
    }

    /// Clears every link to `removed` from the accounts that hold one, and returns each changed
    /// account's id with its config re-serialized for the host's store.
    ///
    /// Every stored link to it goes, not only the ones that currently hold: an account added
    /// again later under the same id must not be picked up by a link nobody made to it.
    pub(crate) fn clear_links_to(&self, removed: &str) -> Changed {
        let mut entries = self
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        let mut changed = Vec::new();
        for (id, entry) in entries.iter_mut() {
            let links = &mut entry.shape_mut().links;
            let mut cleared = false;
            for slot in [&mut links.calendar, &mut links.contacts, &mut links.mail] {
                if slot
                    .as_ref()
                    .is_some_and(|target| target.as_str() == removed)
                {
                    *slot = None;
                    cleared = true;
                }
            }
            if cleared {
                changed.push((id.clone(), entry.to_toml().map_err(|err| err.to_string())));
            }
        }
        changed
    }

    /// Sets `id`'s link in `slot` to `target`, or clears it, and returns each account it changed.
    ///
    /// Naming a mail account from a calendar also sets that mail account's calendar link, when it
    /// is not this calendar yet: a calendar sends only through a mail account linked to it.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for an unknown account, or a target the slot may not name
    /// ([`candidates`]).
    pub(crate) fn set_link(
        &self,
        id: &str,
        slot: LinkSlot,
        target: Option<&str>,
    ) -> Result<Changed, MailcalError> {
        let refused = |reason: &str| MailcalError::Config(reason.to_owned());
        let mut entries = self
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        let facts: Vec<AccountFacts> = entries.iter().map(|(id, entry)| entry.facts(id)).collect();
        let from = facts
            .iter()
            .position(|account| account.id == id)
            .ok_or_else(|| refused("no such account"))?;
        let links = valid_links(&facts);
        let as_id = |text: &str| {
            AccountId::try_from(text).map_err(|err| MailcalError::Config(err.to_string()))
        };
        let mut writes = vec![(id.to_owned(), slot, None)];
        if let Some(target) = target {
            let to = candidates(&facts, &links, from, slot)
                .into_iter()
                .find(|&candidate| facts[candidate].id == target)
                .ok_or_else(|| refused("that account cannot be linked here"))?;
            writes[0].2 = Some(as_id(target)?);
            let linked_back = links[to]
                .calendar
                .as_ref()
                .is_some_and(|calendar| calendar.as_str() == id);
            if slot == LinkSlot::Mail && !linked_back {
                writes.push((target.to_owned(), LinkSlot::Calendar, Some(as_id(id)?)));
            }
            // A calendar sending through its only mail account does so without naming it; a
            // second mail account linking it would leave it naming none, so the one it sends
            // through is named first.
            if slot == LinkSlot::Calendar
                && let Some(sender) = links[to].mail.as_ref()
                && sender.as_str() != id
                && facts[to].links.mail.as_ref() != Some(sender)
            {
                writes.push((target.to_owned(), LinkSlot::Mail, Some(sender.clone())));
            }
        } else if slot == LinkSlot::Mail {
            // A calendar with one mail account linked to it sends through that one without naming
            // it, so clearing the name alone would change nothing: the link itself goes.
            let senders: Vec<&AccountFacts> = facts
                .iter()
                .zip(&links)
                .filter(|(_, theirs)| {
                    theirs
                        .calendar
                        .as_ref()
                        .is_some_and(|calendar| calendar.as_str() == id)
                })
                .map(|(sender, _)| sender)
                .collect();
            if let [only] = senders.as_slice() {
                writes.push((only.id.clone(), LinkSlot::Calendar, None));
            }
        }
        let mut changed = Vec::new();
        for (account, slot, value) in writes {
            let Some(entry) = entries.get_mut(&account) else {
                continue;
            };
            let links = &mut entry.shape_mut().links;
            match slot {
                LinkSlot::Calendar => links.calendar = value,
                LinkSlot::Contacts => links.contacts = value,
                LinkSlot::Mail => links.mail = value,
            }
            changed.push((account, entry.to_toml().map_err(|err| err.to_string())));
        }
        Ok(changed)
    }
}
