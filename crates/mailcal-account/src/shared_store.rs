//! The stores an account's sign-in opens besides its own that the person chose to use
//! (`docs/shared-mailboxes.md`).
//!
//! Each is stored under the account whose sign-in opens it, as a `[[shared_store]]` table, and is
//! listed as an account of its own under an id derived from that account's. One store is one
//! account, with everything it holds that the account is used for: a group's mailbox and its
//! calendar are one switch, as they are one store. A `[[shared_mailbox]]` table, written before
//! calendars were opened, reads as a store holding mail.

use std::fmt;

use engine_core::ids::{AccountId, IdError, SharedAccountId};
use serde::{Deserialize, Deserializer, de};

/// What a shared store holds that an account may open: its mail or its calendars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SharedUse {
    /// Its mail.
    Mail,
    /// Its calendars.
    Calendar,
}

impl SharedUse {
    /// Both, mail first.
    pub const ALL: [Self; 2] = [Self::Mail, Self::Calendar];

    const fn as_str(self) -> &'static str {
        match self {
            Self::Mail => "mail",
            Self::Calendar => "calendar",
        }
    }
}

/// What separates a shared store's id from the id of the account that opens it.
const SEPARATOR: &str = "#shared:";

/// Whether `listed` is the id of a store the account `parent` is used for besides its own.
#[must_use]
pub fn opened_by(listed: &str, parent: &str) -> bool {
    listed
        .strip_prefix(parent)
        .is_some_and(|rest| rest.starts_with(SEPARATOR))
}

/// Which of mail and calendars a store holds, as the servers that found it said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct SharedUses {
    mail: bool,
    calendar: bool,
}

impl SharedUses {
    /// `kind` alone.
    #[must_use]
    pub const fn of(kind: SharedUse) -> Self {
        Self {
            mail: false,
            calendar: false,
        }
        .with(kind)
    }

    /// These and `kind`.
    #[must_use]
    pub const fn with(self, kind: SharedUse) -> Self {
        match kind {
            SharedUse::Mail => Self { mail: true, ..self },
            SharedUse::Calendar => Self {
                calendar: true,
                ..self
            },
        }
    }

    /// Whether `kind` is among them.
    #[must_use]
    pub const fn contains(self, kind: SharedUse) -> bool {
        match kind {
            SharedUse::Mail => self.mail,
            SharedUse::Calendar => self.calendar,
        }
    }

    /// Those in both.
    #[must_use]
    pub const fn intersect(self, other: Self) -> Self {
        Self {
            mail: self.mail && other.mail,
            calendar: self.calendar && other.calendar,
        }
    }

    /// Those in either.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            mail: self.mail || other.mail,
            calendar: self.calendar || other.calendar,
        }
    }

    /// Whether there are none.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !self.mail && !self.calendar
    }

    /// Each, mail first.
    pub fn iter(self) -> impl Iterator<Item = SharedUse> {
        SharedUse::ALL
            .into_iter()
            .filter(move |kind| self.contains(*kind))
    }
}

impl FromIterator<SharedUse> for SharedUses {
    fn from_iter<I: IntoIterator<Item = SharedUse>>(iter: I) -> Self {
        iter.into_iter().fold(Self::default(), Self::with)
    }
}

impl<'de> Deserialize<'de> for SharedUses {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Names;
        impl<'de> de::Visitor<'de> for Names {
            type Value = SharedUses;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(r#"a list of "mail" and "calendar""#)
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<SharedUses, A::Error> {
                let mut uses = SharedUses::default();
                while let Some(name) = seq.next_element::<String>()? {
                    // One a later build stores is not this build's to open.
                    if let Some(kind) = SharedUse::ALL.into_iter().find(|k| k.as_str() == name) {
                        uses = uses.with(kind);
                    }
                }
                Ok(uses)
            }
        }
        deserializer.deserialize_seq(Names)
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
    /// What the store holds, as the servers that found it said: what the handle may be handed
    /// to. An IMAP account's mailbox server and calendar server each find stores the other knows
    /// nothing of, under handles of their own.
    #[serde(default = "mail_alone")]
    pub holds: SharedUses,
}

/// What a store stored without saying holds: a `[[shared_mailbox]]`, which was a mailbox.
const fn mail_alone() -> SharedUses {
    SharedUses::of(SharedUse::Mail)
}

impl SharedStoreChoice {
    /// The id the store is listed under, derived from the id of the account that opens it, so it
    /// is stable for as long as that account's is. A store stored as a shared mailbox keeps the
    /// id it had.
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
        let holds: Vec<toml::Value> = self.holds.iter().map(|k| k.as_str().into()).collect();
        table.insert("holds".into(), holds.into());
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
            holds: SharedUses::of(SharedUse::Mail).with(SharedUse::Calendar),
        }
    }

    #[test]
    fn a_shared_store_is_listed_under_an_id_of_its_own_derived_from_its_parents() {
        let parent = AccountId::try_from("alice@jmap:mail.example.org").unwrap();
        let first = choice("f").account_id(&parent).unwrap();
        // The id a shared mailbox had, so one stored before calendars keeps its mail.
        assert_eq!(first.as_str(), "alice@jmap:mail.example.org#shared:f");
        assert_ne!(first, choice("g").account_id(&parent).unwrap());
        assert_ne!(first, parent);
        assert!(opened_by(first.as_str(), parent.as_str()));
        assert!(!opened_by(parent.as_str(), parent.as_str()));
    }

    #[test]
    fn what_a_store_holds_is_a_set_over_mail_and_calendars() {
        let both = SharedUses::of(SharedUse::Calendar).with(SharedUse::Mail);
        assert_eq!(both.iter().collect::<Vec<_>>(), SharedUse::ALL);
        let mail = SharedUses::of(SharedUse::Mail);
        assert_eq!(both.intersect(mail), mail);
        assert_eq!(mail.union(SharedUses::of(SharedUse::Calendar)), both);
        assert!(SharedUses::default().is_empty());
        assert_eq!(SharedUse::ALL.into_iter().collect::<SharedUses>(), both);
    }
}
