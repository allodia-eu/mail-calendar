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
/// Returns [`MailcalError::Config`] for a choice without mail, calendar or contacts: an account
/// is used for at least one of them, and colleagues are only ever read beside contacts, so a
/// choice of colleagues alone would ask the provider for nothing it could open.
pub(crate) fn chosen(
    capabilities: Option<Vec<AccountCapability>>,
) -> Result<Option<Capabilities>, MailcalError> {
    let Some(list) = capabilities else {
        return Ok(None);
    };
    let chosen: Capabilities = list.into_iter().map(Capability::from).collect();
    let usable = [Capability::Mail, Capability::Calendar, Capability::Contacts]
        .into_iter()
        .any(|capability| chosen.contains(capability));
    if usable {
        Ok(Some(chosen))
    } else {
        Err(MailcalError::Config(
            "an account is used for at least one of mail, calendar and contacts".to_owned(),
        ))
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
        assert!(matches!(
            chosen(Some(vec![AccountCapability::Colleagues])),
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
