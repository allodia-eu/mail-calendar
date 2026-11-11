//! The stores an account's sign-in opens besides its own that the person chose to use, for mail
//! or for calendars (`docs/shared-mailboxes.md`).
//!
//! Each is stored under the account whose sign-in opens it, as a `[[shared_mailbox]]` or a
//! `[[shared_calendar]]` table, and is listed as an account of its own, used for that alone, under
//! an id derived from that account's. One store chosen for both is two accounts: a switch per use.

use engine_core::ids::{AccountId, IdError, SharedAccountId};
use serde::Deserialize;

/// What a store an account's sign-in opens is chosen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SharedUse {
    /// Its mail: a shared mailbox.
    Mail,
    /// Its calendars: a shared calendar.
    Calendar,
}

impl SharedUse {
    /// What separates the id of a store chosen for this use from the id of the account that
    /// opens it.
    const fn separator(self) -> &'static str {
        match self {
            Self::Mail => "#shared:",
            Self::Calendar => "#shared-calendar:",
        }
    }

    /// The name of the table each choice for this use is stored as.
    pub(crate) const fn table(self) -> &'static str {
        match self {
            Self::Mail => "shared_mailbox",
            Self::Calendar => "shared_calendar",
        }
    }
}

/// One store an account is used for, beside its own.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SharedStoreChoice {
    /// The server's handle for the store, handed back to the adapter that produced it.
    pub handle: SharedAccountId,
    /// The server's label for it.
    pub name: String,
    /// The address the server gives for it, when it gives one. A mailbox with an address sends
    /// as that address.
    #[serde(default)]
    pub address: Option<String>,
}

impl SharedStoreChoice {
    /// The id the store is listed under when chosen for `kind`, derived from the id of the
    /// account that opens it, so it is stable for as long as that account's is.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] when the two together are longer than an id may be.
    pub fn account_id(&self, parent: &AccountId, kind: SharedUse) -> Result<AccountId, IdError> {
        AccountId::try_from(format!("{parent}{}{}", kind.separator(), self.handle).as_str())
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

    fn choice(handle: &str) -> SharedStoreChoice {
        SharedStoreChoice {
            handle: SharedAccountId::try_from(handle).unwrap(),
            name: "Support".into(),
            address: Some("support@example.org".into()),
        }
    }

    #[test]
    fn a_shared_mailbox_is_listed_under_an_id_of_its_own_derived_from_its_parents() {
        let parent = AccountId::try_from("alice@jmap:mail.example.org").unwrap();
        let first = choice("f").account_id(&parent, SharedUse::Mail).unwrap();
        assert_eq!(first.as_str(), "alice@jmap:mail.example.org#shared:f");
        assert_ne!(
            first,
            choice("g").account_id(&parent, SharedUse::Mail).unwrap()
        );
        assert_ne!(first, parent);
    }

    #[test]
    fn one_store_chosen_for_mail_and_for_calendars_is_two_accounts() {
        let parent = AccountId::try_from("alice@jmap:mail.example.org").unwrap();
        let calendar = choice("f")
            .account_id(&parent, SharedUse::Calendar)
            .unwrap();
        assert_eq!(
            calendar.as_str(),
            "alice@jmap:mail.example.org#shared-calendar:f"
        );
        assert_ne!(
            calendar,
            choice("f").account_id(&parent, SharedUse::Mail).unwrap()
        );
    }
}
