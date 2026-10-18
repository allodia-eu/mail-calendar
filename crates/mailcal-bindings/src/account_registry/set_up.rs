//! Whether an account is already set up: one mailbox registered under two ids.

use std::collections::HashMap;

use engine_api::AccountId;

use super::{AccountRegistry, Registered};
use crate::ConnectedAccount;

/// The account other than `except` that is the mailbox `ids` name: keyed by one of them, or
/// whose own servers derive one (its id pinned before an edit of its own).
pub(super) fn set_up_as(
    entries: &HashMap<String, ConnectedAccount>,
    ids: &[&AccountId],
    except: Option<&str>,
) -> Option<String> {
    entries
        .iter()
        .find(|(other, entry)| {
            Some(other.as_str()) != except
                && ids.iter().any(|id| {
                    other.as_str() == id.as_str()
                        || entry.derived_account_id().as_ref() == Some(*id)
                })
        })
        .map(|(other, _)| other.clone())
}

impl AccountRegistry {
    /// The account already set up as the mailbox `id` and `derived` name, if any.
    pub(crate) fn already_set_up(
        &self,
        id: &AccountId,
        derived: Option<&AccountId>,
    ) -> Option<String> {
        let entries = self
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        set_up_as(&entries, &ids(id, derived), None)
    }

    /// [`AccountRegistry::pre_register`] for an account that must not be set up already, checked
    /// under the same lock that inserts it, so two adds of one mailbox cannot both pass.
    ///
    /// # Errors
    ///
    /// Returns the id of the account already set up, and registers nothing.
    pub(crate) fn pre_register_new(
        &self,
        id: &AccountId,
        derived: Option<&AccountId>,
        entry: ConnectedAccount,
    ) -> Result<Registered, String> {
        let key = id.as_str().to_owned();
        {
            let mut entries = self
                .entries
                .lock()
                .expect("account registry mutex poisoned");
            if let Some(existing) = set_up_as(&entries, &ids(id, derived), None) {
                return Err(existing);
            }
            entries.insert(key.clone(), entry);
        }
        self.forget_answers(&key);
        Ok(Registered {
            id: key,
            replaced: None,
        })
    }
}

fn ids<'a>(id: &'a AccountId, derived: Option<&'a AccountId>) -> Vec<&'a AccountId> {
    std::iter::once(id).chain(derived).collect()
}
