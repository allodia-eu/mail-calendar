//! What Settings → Accounts says about an account, decided from its snapshot entry alone, so the
//! rules are tested without a window.

use mailcal_bindings::{
    AccountCapability, AccountEntry, AccountKind, CapabilityState, LinkSlot, LinkedAccount,
};

use crate::l10n;

/// The label for an account's kind, as the setup screen names the same choice.
pub(super) fn kind_label(kind: AccountKind) -> &'static str {
    match kind {
        AccountKind::Imap => l10n::setup_account_type_password(),
        AccountKind::Dav => l10n::account_kind_dav(),
        AccountKind::Jmap => l10n::setup_account_type_jmap(),
        AccountKind::Microsoft => l10n::setup_account_type_microsoft(),
        AccountKind::Google => l10n::setup_account_type_google(),
    }
}

/// A use's name, as the main window's sections name it.
pub(super) fn use_name(capability: AccountCapability) -> &'static str {
    match capability {
        AccountCapability::Mail => l10n::nav_mail(),
        AccountCapability::Calendar => l10n::nav_calendar(),
        AccountCapability::Contacts => l10n::nav_contacts(),
        AccountCapability::Colleagues => l10n::account_use_colleagues(),
    }
}

/// The list row's subtitle: the kind, what the account is used for, what it is linked to, and
/// whether something needs the person.
pub(super) fn summary(entry: &AccountEntry) -> String {
    let uses = entry
        .uses
        .iter()
        .filter(|use_| use_.state != CapabilityState::Off)
        .map(|use_| use_name(use_.capability))
        .collect::<Vec<_>>()
        .join(", ");
    let mut lines = vec![format!("{} · {uses}", kind_label(entry.kind))];
    let linked = [
        (AccountCapability::Calendar, &entry.links.calendar),
        (AccountCapability::Contacts, &entry.links.contacts),
        (AccountCapability::Mail, &entry.links.mail),
    ];
    lines.extend(linked.into_iter().filter_map(|(capability, link)| {
        link.as_ref()
            .map(|link| l10n::settings_account_linked_line(use_name(capability), &link.address))
    }));
    if needs_permission(entry) {
        lines.push(l10n::settings_account_needs_permission().to_owned());
    }
    lines.join("\n")
}

/// Whether a use the account is used for waits on the provider's permission.
pub(super) fn needs_permission(entry: &AccountEntry) -> bool {
    entry
        .uses
        .iter()
        .any(|use_| use_.state == CapabilityState::NeedsPermission)
}

/// Whether the account signs in at its provider's own page, which "Sign in again" opens.
pub(super) const fn signs_in_at_provider(kind: AccountKind) -> bool {
    matches!(kind, AccountKind::Microsoft | AccountKind::Google)
}

/// What the sign-in group says beneath its heading: what is wrong, when something is.
pub(super) fn signin_description(entry: &AccountEntry, expired: bool) -> String {
    if expired {
        l10n::signin_expired_prompt(&entry.address)
    } else if needs_permission(entry) {
        l10n::settings_account_needs_permission().to_owned()
    } else {
        l10n::settings_account_signin_description().to_owned()
    }
}

/// How one use's switch is drawn.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct UseSwitch {
    /// On. A use waiting on a permission is drawn off: it is not working, and switching it on
    /// is what asks the provider again.
    pub(super) active: bool,
    /// Whether switching it on asks the provider rather than the core.
    pub(super) asks: bool,
    /// Whether the person can flip it.
    pub(super) sensitive: bool,
    /// The line beneath it, when there is something to say.
    pub(super) note: Option<&'static str>,
}

/// The switch for `capability` on `entry`. The last of mail, calendar and contacts cannot be
/// switched off, and colleagues cannot be switched on without contacts: the core refuses both, so
/// the switch says why rather than letting the person find out.
pub(super) fn use_switch(entry: &AccountEntry, capability: AccountCapability) -> UseSwitch {
    let used = |wanted| {
        entry
            .uses
            .iter()
            .any(|use_| use_.capability == wanted && use_.state != CapabilityState::Off)
    };
    let state = entry
        .uses
        .iter()
        .find(|use_| use_.capability == capability)
        .map_or(CapabilityState::Off, |use_| use_.state);
    let active = state == CapabilityState::On;
    let primary = [
        AccountCapability::Mail,
        AccountCapability::Calendar,
        AccountCapability::Contacts,
    ];
    let last = state != CapabilityState::Off
        && primary.contains(&capability)
        && primary.iter().filter(|wanted| used(**wanted)).count() == 1;
    let without_contacts = capability == AccountCapability::Colleagues
        && state == CapabilityState::Off
        && !used(AccountCapability::Contacts);
    let note = if last {
        Some(l10n::settings_account_use_last())
    } else if without_contacts {
        Some(l10n::settings_account_colleagues_needs_contacts())
    } else if state == CapabilityState::NeedsPermission {
        Some(l10n::settings_account_use_withheld())
    } else {
        None
    };
    UseSwitch {
        active,
        asks: state == CapabilityState::NeedsPermission,
        sensitive: !last && !without_contacts,
        note,
    }
}

/// One link slot's picker: "none" first, then the accounts it may name.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct LinkPicker {
    pub(super) slot: LinkSlot,
    pub(super) title: &'static str,
    pub(super) options: Vec<LinkedAccount>,
    /// The index into `options` of the account linked now; `None` for "none".
    pub(super) selected: Option<usize>,
    /// The index into `options` of the account to suggest, while nothing is linked: one whose
    /// calendar server schedules as the mail account's address.
    pub(super) suggested: Option<usize>,
}

/// The pickers an account's page offers: one per slot it can hold, which is a slot with
/// candidates or one already linked.
pub(super) fn link_pickers(entry: &AccountEntry) -> Vec<LinkPicker> {
    [
        (
            LinkSlot::Calendar,
            l10n::settings_account_link_calendar(),
            &entry.link_candidates.calendar,
            &entry.links.calendar,
        ),
        (
            LinkSlot::Contacts,
            l10n::settings_account_link_contacts(),
            &entry.link_candidates.contacts,
            &entry.links.contacts,
        ),
        (
            LinkSlot::Mail,
            l10n::settings_account_link_mail(),
            &entry.link_candidates.mail,
            &entry.links.mail,
        ),
    ]
    .into_iter()
    .filter(|(_, _, candidates, current)| !candidates.is_empty() || current.is_some())
    .map(|(slot, title, candidates, current)| {
        let mut options = candidates.clone();
        if let Some(current) = current
            && !options.contains(current)
        {
            options.insert(0, current.clone());
        }
        let selected = current
            .as_ref()
            .and_then(|current| options.iter().position(|option| option == current));
        let suggested = selected.is_none().then(|| {
            options
                .iter()
                .position(|option| entry.link_candidates.suggested.contains(&option.id))
        });
        LinkPicker {
            slot,
            title,
            options,
            selected,
            suggested: suggested.flatten(),
        }
    })
    .collect()
}

#[cfg(test)]
#[path = "account_rows_tests.rs"]
mod tests;
