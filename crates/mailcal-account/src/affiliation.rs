//! What a Microsoft or Google account can be used for, given who it belongs to.

use engine_api::Affiliation;

use crate::{Capabilities, Capability};

/// Whose accounts a provider makes video meetings for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MeetingsFor {
    /// An organisation's accounts only: Microsoft makes an online meeting for no personal one.
    Organisations,
    /// Every account: Google makes a Meet space for a personal one too.
    Everyone,
}

impl MeetingsFor {
    /// Whether an account, `personal` or not, is offered the use.
    pub(crate) const fn offers(self, personal: bool) -> bool {
        match self {
            Self::Organisations => !personal,
            Self::Everyone => true,
        }
    }
}

/// Every use, less colleagues and shared mailboxes for a personal account, which has no
/// organisation, so no directory and no mailbox shared within one, and less meetings where the
/// provider makes none for it (`meetings`): each would wait on a permission no sign-in can
/// give. Shared mailboxes only where this app opens them through the provider
/// (`shared_mailboxes`). An account whose affiliation is not known yet is offered everything,
/// as before it was asked.
#[must_use]
pub(crate) fn offered(
    affiliation: Option<&Affiliation>,
    shared_mailboxes: bool,
    meetings: MeetingsFor,
) -> Capabilities {
    let personal = affiliation == Some(&Affiliation::Personal);
    Capability::ALL
        .into_iter()
        .filter(|capability| match capability {
            Capability::Colleagues => !personal,
            Capability::SharedMailboxes => shared_mailboxes && !personal,
            Capability::Meetings => meetings.offers(personal),
            _ => true,
        })
        .collect()
}

/// `chosen`, less what `offered` leaves out.
#[must_use]
pub(crate) fn within(chosen: &Capabilities, offered: &Capabilities) -> Capabilities {
    chosen
        .iter()
        .filter(|capability| offered.contains(*capability))
        .collect()
}
