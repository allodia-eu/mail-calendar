//! What an account is used for, and which other accounts supply what it lacks: the part of a
//! stored config every kind shares, beside its own endpoints and credential.
//!
//! It lives at the root of the account's TOML document, next to the kind's own sections
//! (`[imap]`, `[microsoft]`, `[google]`, `[jmap]`), and is read and written the same way for
//! all four. Every key is optional and written only when set, so a document stored before
//! these keys existed reads unchanged and serializes back byte for byte.

use std::collections::BTreeSet;

use engine_core::ids::AccountId;
use serde::Deserialize;

use crate::ConfigError;

/// One thing an account can be used for.
///
/// `Colleagues` is a sub-choice of `Contacts` on a Microsoft or Google account: the people in
/// the person's work or school directory, which a provider grants as a separate permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// Reading and sending mail.
    Mail,
    /// The account's calendar.
    Calendar,
    /// The account's own contacts.
    Contacts,
    /// The organisation's directory, beside the account's own contacts.
    Colleagues,
}

impl Capability {
    /// Every capability, in the order they are listed.
    pub const ALL: [Self; 4] = [Self::Mail, Self::Calendar, Self::Contacts, Self::Colleagues];

    /// The name the stored config spells it with.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mail => "mail",
            Self::Calendar => "calendar",
            Self::Contacts => "contacts",
            Self::Colleagues => "colleagues",
        }
    }

    /// The capability a stored name spells, or `None` for one this build does not know.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.as_str() == name)
    }
}

/// The capabilities an account is used for, as stored.
///
/// A name this build does not know is kept rather than refused or dropped: refusing it would
/// leave an account a newer build wrote unreadable here, and dropping it would rewrite the
/// account as used for less than it is on the next save. It counts for nothing in this build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities {
    known: BTreeSet<Capability>,
    unknown: BTreeSet<String>,
}

impl Capabilities {
    /// Whether the account is used for `capability`.
    #[must_use]
    pub fn contains(&self, capability: Capability) -> bool {
        self.known.contains(&capability)
    }

    /// The capabilities this build knows, in a fixed order.
    pub fn iter(&self) -> impl Iterator<Item = Capability> + '_ {
        self.known.iter().copied()
    }

    /// Whether nothing at all is listed, known or not.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.known.is_empty() && self.unknown.is_empty()
    }

    /// The stored names: the known ones in their fixed order, then any others as they were.
    fn names(&self) -> impl Iterator<Item = &str> {
        self.known
            .iter()
            .map(|capability| capability.as_str())
            .chain(self.unknown.iter().map(String::as_str))
    }
}

impl FromIterator<Capability> for Capabilities {
    fn from_iter<I: IntoIterator<Item = Capability>>(iter: I) -> Self {
        Self {
            known: iter.into_iter().collect(),
            unknown: BTreeSet::new(),
        }
    }
}

impl<'de> Deserialize<'de> for Capabilities {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut capabilities = Self::default();
        for name in Vec::<String>::deserialize(deserializer)? {
            match Capability::parse(&name) {
                Some(capability) => {
                    capabilities.known.insert(capability);
                }
                None => {
                    capabilities.unknown.insert(name);
                }
            }
        }
        Ok(capabilities)
    }
}

/// The accounts this one relies on for what it does not do itself.
///
/// A mail account names the calendar that files and answers its invitations and the address
/// book new contacts go to; a calendar account names which of the mail accounts linked to it
/// sends its invitations. Each target is another account's id. Whether a link still points
/// anywhere is decided when it is read, against the accounts that exist then.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct AccountLinks {
    /// The calendar account a mail account files and answers invitations through.
    #[serde(default)]
    pub calendar: Option<AccountId>,
    /// The account whose address book a mail account's new contacts are saved to.
    #[serde(default)]
    pub contacts: Option<AccountId>,
    /// The mail account a calendar account sends its invitations through.
    #[serde(default)]
    pub mail: Option<AccountId>,
}

impl AccountLinks {
    /// Whether no link is set.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.calendar.is_none() && self.contacts.is_none() && self.mail.is_none()
    }
}

/// The shared part of a stored account.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct AccountShape {
    /// The account's id, once it has been pinned. Absent, the id is derived from the account's
    /// own settings as it always was; stored, it survives an edit of the settings it was
    /// derived from.
    #[serde(default)]
    pub id: Option<AccountId>,
    /// What the account is used for. Absent means the meaning every account had before the
    /// choice existed, which each kind states for itself.
    #[serde(default, deserialize_with = "non_empty")]
    pub capabilities: Option<Capabilities>,
    /// The accounts this one relies on.
    #[serde(default)]
    pub links: AccountLinks,
}

impl AccountShape {
    /// Reads the shared keys from an account's stored TOML, whatever its kind; the kind's own
    /// sections are left to its loader.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Parse`] if the document is not TOML, or if one of these keys holds
    /// something it cannot (an id that is not one, a capability this build does not know).
    pub fn read(text: &str) -> Result<Self, ConfigError> {
        Ok(toml::from_str(text)?)
    }

    /// What the account is used for: what it stores, or `legacy` when it stores nothing, which
    /// is what every account meant before the choice existed and is the kind's to state.
    #[must_use]
    pub fn capabilities_or(&self, legacy: impl IntoIterator<Item = Capability>) -> Capabilities {
        self.capabilities
            .clone()
            .unwrap_or_else(|| legacy.into_iter().collect())
    }

    /// Adds the keys that are set to an account document's root table. Nothing is added for an
    /// account that has none, which is what keeps an older document byte for byte.
    pub(crate) fn write_into(&self, root: &mut toml::Table) {
        if let Some(id) = &self.id {
            root.insert("id".into(), id.as_str().to_owned().into());
        }
        if let Some(capabilities) = &self.capabilities {
            let names: Vec<toml::Value> = capabilities.names().map(Into::into).collect();
            root.insert("capabilities".into(), names.into());
        }
        if !self.links.is_empty() {
            let mut links = toml::Table::new();
            for (slot, target) in [
                ("calendar", &self.links.calendar),
                ("contacts", &self.links.contacts),
                ("mail", &self.links.mail),
            ] {
                if let Some(target) = target {
                    links.insert(slot.into(), target.as_str().to_owned().into());
                }
            }
            root.insert("links".into(), links.into());
        }
    }
}

/// An empty capability list reads as absent: an account is used for at least one thing, or it
/// is removed rather than emptied, so an empty list is a document nobody should have written,
/// and reading it as the older meaning keeps the account working.
fn non_empty<'de, D>(deserializer: D) -> Result<Option<Capabilities>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let capabilities = Option::<Capabilities>::deserialize(deserializer)?;
    Ok(capabilities.filter(|set| !set.is_empty()))
}

#[cfg(test)]
#[path = "account_shape_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "account_shape_kinds_tests.rs"]
mod kinds_tests;
