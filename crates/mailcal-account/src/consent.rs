//! What a Microsoft or Google sign-in asks for, from what the account is used for, and which of
//! those uses the grant it came back with withholds.
//!
//! The scopes themselves are [`mailcal_oauth::scopes`]'; this is the mapping from a
//! [`Capability`] onto them, which is the product's half.

use mailcal_oauth::scopes::{ProviderScopes, ScopeGroup};

use crate::{Capabilities, Capability};

/// The scope group a capability asks for at `provider`.
const fn group(provider: &ProviderScopes, capability: Capability) -> &ScopeGroup {
    match capability {
        Capability::Mail => &provider.mail,
        Capability::Calendar => &provider.calendar,
        Capability::Contacts => &provider.contacts,
        Capability::Colleagues => &provider.colleagues,
    }
}

/// The uses whose scopes are asked for: colleagues only beside contacts, because the directory
/// is only ever read as part of the address book.
fn asked(capabilities: &Capabilities) -> impl Iterator<Item = Capability> + '_ {
    capabilities.iter().filter(|capability| {
        *capability != Capability::Colleagues || capabilities.contains(Capability::Contacts)
    })
}

/// The scopes a sign-in for an account used for `capabilities` requests at `provider`.
#[must_use]
pub fn requested_scopes(provider: &ProviderScopes, capabilities: &Capabilities) -> Vec<String> {
    let address = if capabilities.contains(Capability::Mail) {
        &[][..]
    } else {
        provider.address_without_mail
    };
    provider
        .always
        .iter()
        .chain(asked(capabilities).flat_map(|capability| group(provider, capability).requested))
        .chain(address)
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
        .filter(|capability| !provider.holds(granted, group(provider, *capability).needed))
        .collect()
}

#[cfg(test)]
#[path = "consent_tests.rs"]
mod tests;
