//! The mailboxes an account's sign-in opens besides its own that the person chose to use
//! (`docs/shared-mailboxes.md`).
//!
//! Each is stored under the account whose sign-in opens it, as a `[[shared_mailbox]]` table, and
//! is listed everywhere as a mail account of its own, under an id derived from that account's.

use engine_core::ids::{AccountId, IdError, SharedMailboxId};
use serde::Deserialize;

/// What separates a shared mailbox's id from the id of the account that opens it.
const SEPARATOR: &str = "#shared:";

/// One shared mailbox an account is used for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SharedMailboxChoice {
    /// The server's handle for the mailbox, handed back to the adapter that produced it.
    pub handle: SharedMailboxId,
    /// The server's label for it.
    pub name: String,
    /// The address the server gives for it, when it gives one. A mailbox with an address sends
    /// as that address.
    #[serde(default)]
    pub address: Option<String>,
}

impl SharedMailboxChoice {
    /// The id the mailbox is listed under, derived from the id of the account that opens it, so
    /// it is stable for as long as that account's is.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] when the two together are longer than an id may be.
    pub fn account_id(&self, parent: &AccountId) -> Result<AccountId, IdError> {
        AccountId::try_from(format!("{parent}{SEPARATOR}{}", self.handle).as_str())
    }

    pub(crate) fn write_into(&self) -> toml::Table {
        let mut table = toml::Table::new();
        table.insert("handle".into(), self.handle.as_str().to_owned().into());
        table.insert("name".into(), self.name.clone().into());
        if let Some(address) = &self.address {
            table.insert("address".into(), address.clone().into());
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(handle: &str) -> SharedMailboxChoice {
        SharedMailboxChoice {
            handle: SharedMailboxId::try_from(handle).unwrap(),
            name: "Support".into(),
            address: Some("support@example.org".into()),
        }
    }

    #[test]
    fn a_shared_mailbox_is_listed_under_an_id_of_its_own_derived_from_its_parents() {
        let parent = AccountId::try_from("alice@jmap:mail.example.org").unwrap();
        let first = choice("f").account_id(&parent).unwrap();
        assert_eq!(first.as_str(), "alice@jmap:mail.example.org#shared:f");
        assert_ne!(first, choice("g").account_id(&parent).unwrap());
        assert_ne!(first, parent);
    }
}
