//! What a Microsoft or Google sign-in asks for, from what the account is used for, and which of
//! those uses the grant it came back with withholds.
//!
//! The scopes themselves are [`mailcal_oauth::scopes`]'; this is the mapping from a
//! [`Capability`] onto them, which is the product's half.

use mailcal_oauth::scopes::{ProviderScopes, ScopeGroup};

use crate::{Capabilities, Capability};

/// The scope group a capability asks for at `provider`, `None` for one it has no scopes for.
const fn group(provider: &ProviderScopes, capability: Capability) -> Option<&ScopeGroup> {
    match capability {
        Capability::Mail => Some(&provider.mail),
        Capability::Calendar => Some(&provider.calendar),
        Capability::Contacts => Some(&provider.contacts),
        Capability::Colleagues => Some(&provider.colleagues),
        Capability::SharedMailboxes => provider.shared_mailboxes.as_ref(),
    }
}

/// The uses whose scopes are asked for: colleagues only beside contacts, because the directory
/// is only ever read as part of the address book, and shared mailboxes only beside mail.
fn asked(capabilities: &Capabilities) -> impl Iterator<Item = Capability> + '_ {
    capabilities.iter().filter(|capability| match capability {
        Capability::Colleagues => capabilities.contains(Capability::Contacts),
        Capability::SharedMailboxes => capabilities.contains(Capability::Mail),
        _ => true,
    })
}

/// The scopes a sign-in for an account used for `capabilities` requests at `provider`.
#[must_use]
pub fn requested_scopes(provider: &ProviderScopes, capabilities: &Capabilities) -> Vec<String> {
    provider
        .always
        .iter()
        .chain(
            asked(capabilities)
                .filter_map(|capability| group(provider, capability))
                .flat_map(|group| group.requested),
        )
        .map(|scope| (*scope).to_owned())
        .collect()
}

/// The uses in `capabilities` that `granted` does not allow: chosen, asked for, and refused.
///
/// `granted` is `None` for a grant stored before the granted set was recorded, which withholds
/// nothing until a refresh says otherwise. A use is withheld only when its **needed** scope is
/// missing; a missing scope that serves one feature within it is refused when that feature is
/// reached.
#[must_use]
pub fn withheld(
    provider: &ProviderScopes,
    capabilities: &Capabilities,
    granted: Option<&[String]>,
) -> Capabilities {
    let Some(granted) = granted else {
        return Capabilities::default();
    };
    asked(capabilities)
        .filter(|capability| {
            group(provider, *capability).is_some_and(|group| !provider.holds(granted, group.needed))
        })
        .collect()
}

#[cfg(test)]
#[path = "consent_tests.rs"]
mod tests;
