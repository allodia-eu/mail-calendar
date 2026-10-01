//! What the registry tells Settings → Accounts, and the links it clears when an account goes.

use super::AccountRegistry;
use crate::accounts_view::AccountFacts;

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

    /// Clears every link to `removed` from the accounts that hold one, and returns each changed
    /// account's id with its config re-serialized for the host's store.
    ///
    /// Every stored link to it goes, not only the ones that currently hold: an account added
    /// again later under the same id must not be picked up by a link nobody made to it.
    pub(crate) fn clear_links_to(&self, removed: &str) -> Vec<(String, Result<String, String>)> {
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
}
