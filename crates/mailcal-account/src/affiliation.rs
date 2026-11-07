//! What a Microsoft or Google account can be used for, given who it belongs to.

use engine_api::Affiliation;

use crate::{Capabilities, Capability};

/// Every use, less colleagues, shared mailboxes and meetings for a personal account: it has no
/// organisation, so no directory and no mailbox shared within one, and Microsoft makes an online
/// meeting only for an organisation's account, so each use would wait on a permission no sign-in
/// can give. Shared mailboxes and meetings only where this app opens them through the provider
/// (`shared_mailboxes`, `meetings`). An account whose affiliation is not known yet is offered
/// everything, as before it was asked.
#[must_use]
pub(crate) fn offered(
    affiliation: Option<&Affiliation>,
    shared_mailboxes: bool,
    meetings: bool,
) -> Capabilities {
    let personal = affiliation == Some(&Affiliation::Personal);
    Capability::ALL
        .into_iter()
        .filter(|capability| match capability {
            Capability::Colleagues => !personal,
            Capability::SharedMailboxes => shared_mailboxes && !personal,
            Capability::Meetings => meetings && !personal,
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
