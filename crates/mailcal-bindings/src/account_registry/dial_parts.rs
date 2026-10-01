//! What a dial makes of the capabilities it tried: which failure is the account's, and which
//! only costs one surface its contents.
//!
//! An account opens each capability it is used for on its own. Mail, when the account is used
//! for it, is still what decides whether the account connected: a mailbox that cannot be reached
//! is an account that cannot be reached, whatever its calendar did. An account without mail is
//! connected when anything it is used for connected, and reports the first failure when nothing
//! did, so a calendar-only account can say its server is unreachable or its sign-in has expired.

use super::dial::ConnectFailure;

/// What one capability of a dial came back with.
pub(crate) enum Part<T> {
    /// The account is not used for it, so nothing was tried.
    Off,
    /// It connected, with these providers (possibly none: an account may have no calendar).
    Bound(Vec<T>),
    /// It was tried and failed.
    Failed(ConnectFailure),
}

impl<T> From<Result<Vec<T>, ConnectFailure>> for Part<T> {
    fn from(result: Result<Vec<T>, ConnectFailure>) -> Self {
        match result {
            Ok(providers) => Self::Bound(providers),
            Err(failure) => Self::Failed(failure),
        }
    }
}

impl<T> Part<T> {
    const fn connected(&self) -> bool {
        matches!(self, Self::Bound(_))
    }

    /// The providers, and the failure when there was one; an optional capability's failure
    /// leaves its surface empty rather than failing the account.
    fn split(self) -> (Vec<T>, Option<ConnectFailure>) {
        match self {
            Self::Off => (Vec::new(), None),
            Self::Bound(providers) => (providers, None),
            Self::Failed(failure) => (Vec::new(), Some(failure)),
        }
    }
}

/// A dial's capabilities, assembled into what the account binds.
#[derive(Debug)]
pub(crate) struct Assembled<M, C, K> {
    pub(crate) mail: Vec<M>,
    pub(crate) calendar: Vec<C>,
    pub(crate) contacts: Vec<K>,
    /// Why the calendar is empty, when it was tried and failed.
    pub(crate) calendar_failure: Option<ConnectFailure>,
}

/// Decides what the account binds from what each capability came back with.
///
/// # Errors
///
/// Returns the mail failure when the account is used for mail and mail failed; for an account
/// without mail, the first failure (calendar, then contacts) when nothing it is used for
/// connected.
pub(crate) fn assemble<M, C, K>(
    mail: Part<M>,
    calendar: Part<C>,
    contacts: Part<K>,
) -> Result<Assembled<M, C, K>, ConnectFailure> {
    let mail = match mail {
        Part::Failed(failure) => return Err(failure),
        Part::Bound(providers) => Some(providers),
        Part::Off => None,
    };
    let mail = match mail {
        Some(providers) => providers,
        None if calendar.connected() || contacts.connected() => Vec::new(),
        None => {
            let (_, calendar_failure) = calendar.split();
            let (_, contacts_failure) = contacts.split();
            return match calendar_failure.or(contacts_failure) {
                Some(failure) => Err(failure),
                // Used for nothing this build can open: an account with no providers, rather
                // than a failure nobody could act on.
                None => Ok(Assembled {
                    mail: Vec::new(),
                    calendar: Vec::new(),
                    contacts: Vec::new(),
                    calendar_failure: None,
                }),
            };
        }
    };
    let (calendar, calendar_failure) = calendar.split();
    let (contacts, _) = contacts.split();
    Ok(Assembled {
        mail,
        calendar,
        contacts,
        calendar_failure,
    })
}

#[cfg(test)]
#[path = "dial_parts_tests.rs"]
mod tests;
