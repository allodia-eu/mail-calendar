//! What an account is used for, as a client names it: the FFI face of
//! [`mailcal_account::Capability`].

use mailcal_account::{Capabilities, Capability};

use crate::MailcalError;

/// One thing an account can be used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccountCapability {
    /// Reading and sending mail.
    Mail,
    /// The account's calendar.
    Calendar,
    /// The account's own contacts.
    Contacts,
    /// The organisation's directory, beside the account's own contacts, on Microsoft and Google.
    Colleagues,
}

impl From<AccountCapability> for Capability {
    fn from(capability: AccountCapability) -> Self {
        match capability {
            AccountCapability::Mail => Self::Mail,
            AccountCapability::Calendar => Self::Calendar,
            AccountCapability::Contacts => Self::Contacts,
            AccountCapability::Colleagues => Self::Colleagues,
        }
    }
}

/// The capabilities a client chose, or `None` when it named none, which keeps what every sign-in
/// meant before the choice existed: everything.
///
/// # Errors
///
/// Returns [`MailcalError::Config`] for an empty choice: an account is used for at least one
/// thing, and a client offering the choice keeps one selected.
pub(crate) fn chosen(
    capabilities: Option<Vec<AccountCapability>>,
) -> Result<Option<Capabilities>, MailcalError> {
    match capabilities {
        None => Ok(None),
        Some(list) if list.is_empty() => Err(MailcalError::Config(
            "an account is used for at least one of mail, calendar and contacts".to_owned(),
        )),
        Some(list) => Ok(Some(list.into_iter().map(Capability::from).collect())),
    }
}

/// The stored names of a choice, for the `pending` handle a sign-in round-trips.
pub(crate) fn names(capabilities: &Capabilities) -> Vec<String> {
    capabilities
        .iter()
        .map(|capability| capability.as_str().to_owned())
        .collect()
}

/// A choice read back from its names; one this build does not know counts for nothing.
pub(crate) fn from_names(names: &[String]) -> Capabilities {
    names
        .iter()
        .filter_map(|name| Capability::parse(name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_choice_keeps_the_older_meaning_and_an_empty_one_is_refused() {
        assert!(chosen(None).unwrap().is_none());
        assert!(matches!(
            chosen(Some(Vec::new())),
            Err(MailcalError::Config(_))
        ));
    }

    #[test]
    fn a_choice_survives_the_pending_handle() {
        let choice = chosen(Some(vec![
            AccountCapability::Contacts,
            AccountCapability::Calendar,
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(from_names(&names(&choice)), choice);
    }
}
