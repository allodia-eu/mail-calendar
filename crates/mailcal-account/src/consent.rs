//! What a Microsoft or Google sign-in asks for, from what the account is used for, and which of
//! those uses the grant it came back with withholds.
//!
//! The scopes themselves are [`mailcal_oauth::scopes`]'; this is the mapping from a
//! [`Capability`] onto them, which is the product's half.

use mailcal_oauth::scopes::{ProviderScopes, ScopeGroup};

use crate::{Capabilities, Capability};

/// The scope groups `capability` asks for at `provider` within `capabilities`: none for one it
/// has no scopes for. Colleagues are asked only beside contacts, because the directory is only
/// ever read as part of the address book; shared mailboxes and calendars ask for the shared mail
/// scopes beside mail and the shared calendar scope beside calendar, since each opens that much of
/// a store.
fn groups<'a>(
    provider: &'a ProviderScopes,
    capabilities: &Capabilities,
    capability: Capability,
) -> Vec<&'a ScopeGroup> {
    let beside = |other| capabilities.contains(other);
    match capability {
        Capability::Mail => vec![&provider.mail],
        Capability::Calendar => vec![&provider.calendar],
        Capability::Contacts => vec![&provider.contacts],
        Capability::Colleagues if beside(Capability::Contacts) => vec![&provider.colleagues],
        Capability::Colleagues => Vec::new(),
        Capability::SharedMailboxes => [
            (Capability::Mail, provider.shared_mailboxes.as_ref()),
            (Capability::Calendar, provider.shared_calendars.as_ref()),
        ]
        .into_iter()
        .filter(|(with, _)| beside(*with))
        .filter_map(|(_, group)| group)
        .collect(),
        Capability::Meetings => provider.meetings.iter().collect(),
    }
}

/// The scopes a sign-in for an account used for `capabilities` requests at `provider`.
#[must_use]
pub fn requested_scopes(provider: &ProviderScopes, capabilities: &Capabilities) -> Vec<String> {
    provider
        .always
        .iter()
        .chain(
            capabilities
                .iter()
                .flat_map(|capability| groups(provider, capabilities, capability))
                .flat_map(|group| group.requested),
        )
        .map(|scope| (*scope).to_owned())
        .collect()
}

/// The uses in `capabilities` that `granted` does not allow: chosen, asked for, and refused.
///
/// `granted` is `None` for a grant stored before the granted set was recorded, which withholds
/// nothing until a refresh says otherwise. A use is withheld only when it asks for scopes and
/// none of their **needed** scopes is held; a missing scope that serves one feature within it is
/// refused when that feature is reached. So a grant from before shared calendars, holding the
/// shared mail scope alone, keeps its shared mailboxes, and a store added through it finds no
/// calendar to open.
#[must_use]
pub fn withheld(
    provider: &ProviderScopes,
    capabilities: &Capabilities,
    granted: Option<&[String]>,
) -> Capabilities {
    let Some(granted) = granted else {
        return Capabilities::default();
    };
    capabilities
        .iter()
        .filter(|capability| {
            let groups = groups(provider, capabilities, *capability);
            !groups.is_empty()
                && groups
                    .iter()
                    .all(|group| !provider.holds(granted, group.needed))
        })
        .collect()
}

#[cfg(test)]
#[path = "consent_tests.rs"]
mod tests;
