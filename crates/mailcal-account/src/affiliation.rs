//! What a Microsoft or Google account can be used for, given who it belongs to.

use engine_api::Affiliation;

use crate::{Capabilities, Capability};

/// Every use, less colleagues for a personal account: it has no organisation, so no directory,
/// and neither provider grants one, so the use would wait on a permission no sign-in can give.
/// An account whose affiliation is not known yet is offered everything, as before it was asked.
#[must_use]
pub(crate) fn offered(affiliation: Option<&Affiliation>) -> Capabilities {
    Capability::ALL
        .into_iter()
        .filter(|capability| {
            *capability != Capability::Colleagues || affiliation != Some(&Affiliation::Personal)
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
