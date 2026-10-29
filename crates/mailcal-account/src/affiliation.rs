//! What a Microsoft or Google account can be used for, given who it belongs to.

use engine_api::Affiliation;

use crate::{Capabilities, Capability};

/// Every use, less colleagues and shared mailboxes for a personal account: it has no
/// organisation, so no directory and no mailbox shared within one, and the use would wait on a
/// permission no sign-in can give. Shared mailboxes only where the provider can open one
/// (`shared_mailboxes`). An account whose affiliation is not known yet is offered everything, as
/// before it was asked.
#[must_use]
pub(crate) fn offered(affiliation: Option<&Affiliation>, shared_mailboxes: bool) -> Capabilities {
    let personal = affiliation == Some(&Affiliation::Personal);
    Capability::ALL
        .into_iter()
        .filter(|capability| match capability {
            Capability::Colleagues => !personal,
            Capability::SharedMailboxes => shared_mailboxes && !personal,
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
