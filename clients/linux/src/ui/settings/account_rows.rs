//! What Settings → Accounts says about an account, decided from its snapshot entry alone, so the
//! rules are tested without a window.

use mailcal_bindings::{
    AccountCapability, AccountEntry, AccountKind, CapabilityState, LinkSlot, LinkedAccount,
    UseProblem,
};

use super::account_names::{kind_line, linked_name};
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
pub(in crate::ui) fn use_name(capability: AccountCapability) -> &'static str {
    match capability {
        AccountCapability::Mail => l10n::nav_mail(),
        AccountCapability::Calendar => l10n::nav_calendar(),
        AccountCapability::Contacts => l10n::nav_contacts(),
        AccountCapability::Colleagues => l10n::account_use_colleagues(),
        AccountCapability::SharedMailboxes => l10n::account_use_shared_mailboxes(),
    }
}

/// The list row's subtitle: the kind, what the account is used for, what it is linked to, and
/// whether something needs the person.
pub(super) fn summary(entry: &AccountEntry) -> String {
    let uses = entry
        .uses
        .iter()
        .filter(|use_| works(use_.state))
        .map(|use_| use_name(use_.capability))
        .collect::<Vec<_>>()
        .join(", ");
    let mut lines = vec![format!("{} · {uses}", kind_line(entry))];
    let linked = [
        (AccountCapability::Calendar, &entry.links.calendar),
        (AccountCapability::Contacts, &entry.links.contacts),
        (AccountCapability::Mail, &entry.links.mail),
    ];
    lines.extend(linked.into_iter().filter_map(|(capability, link)| {
        link.as_ref().map(|link| {
            l10n::settings_account_linked_line(use_name(capability), &linked_name(link))
        })
    }));
    if needs_permission(entry) {
        lines.push(l10n::settings_account_needs_permission().to_owned());
    }
    lines.join("\n")
}

/// Whether the account is used for a use in `state`: on, or waiting on the provider's permission.
/// A use the server does not offer is not one, whatever was chosen.
pub(in crate::ui) const fn works(state: CapabilityState) -> bool {
    matches!(
        state,
        CapabilityState::On | CapabilityState::NeedsPermission
    )
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
        l10n::signin_expired_prompt(&super::account_names::entry_name(entry))
    } else if needs_permission(entry) {
        l10n::settings_account_needs_permission().to_owned()
    } else {
        l10n::settings_account_signin_description().to_owned()
    }
}

/// What a use that did not connect says beneath its switch. Only a calendar or an address book
/// fails on its own; mail decides whether the account connected at all.
fn problem_note(capability: AccountCapability, problem: UseProblem) -> &'static str {
    let calendar = matches!(capability, AccountCapability::Calendar);
    match problem {
        UseProblem::Unreachable if calendar => l10n::settings_account_use_unreachable_calendar(),
        UseProblem::Unreachable => l10n::settings_account_use_unreachable_contacts(),
        UseProblem::SignInRefused if calendar => l10n::settings_account_use_refused_calendar(),
        UseProblem::SignInRefused => l10n::settings_account_use_refused_contacts(),
        UseProblem::NeedsOwnSignIn if calendar => l10n::settings_account_use_own_sign_in_calendar(),
        UseProblem::NeedsOwnSignIn => l10n::settings_account_use_own_sign_in_contacts(),
        UseProblem::ServerChanged if calendar => {
            l10n::settings_account_use_server_changed_calendar()
        }
        UseProblem::ServerChanged => l10n::settings_account_use_server_changed_contacts(),
    }
}

/// The use whose row the account page's "Review" sits under: the last of those paused for a
/// server found on another site, so it follows every note that says so. `None` when none is.
pub(super) fn review_after(entry: &AccountEntry) -> Option<AccountCapability> {
    entry
        .uses
        .iter()
        .rfind(|use_| use_.problem == Some(UseProblem::ServerChanged))
        .map(|use_| use_.capability)
}

/// The line above the calendar week or the contacts list for a use that did not connect
/// (`docs/accounts.md` rule 16): which account, why in the words its own page uses, and that what
/// is on screen is what was last synced.
pub(in crate::ui) fn outage_line(
    capability: AccountCapability,
    problem: UseProblem,
    address: &str,
) -> String {
    let title = if matches!(capability, AccountCapability::Calendar) {
        l10n::calendar_outage_title(address)
    } else {
        l10n::contacts_outage_title(address)
    };
    format!(
        "{title} {} {}",
        problem_note(capability, problem),
        l10n::use_outage_last_synced()
    )
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
/// switched off, and colleagues cannot be switched on without contacts, nor shared mailboxes
/// without mail: the core refuses each, so the switch says why rather than letting the person
/// find out.
pub(super) fn use_switch(entry: &AccountEntry, capability: AccountCapability) -> UseSwitch {
    let used = |wanted| {
        entry
            .uses
            .iter()
            .any(|use_| use_.capability == wanted && works(use_.state))
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
    // A use waiting on a permission is drawn off, so it is never the one held on: switching it
    // on asks the provider, which the core does not refuse.
    let last = state == CapabilityState::On
        && primary.contains(&capability)
        && primary.iter().filter(|wanted| used(**wanted)).count() == 1;
    let without = |sub, within| capability == sub && state == CapabilityState::Off && !used(within);
    let without_contacts = without(AccountCapability::Colleagues, AccountCapability::Contacts);
    let without_mail = without(AccountCapability::SharedMailboxes, AccountCapability::Mail);
    let not_offered = state == CapabilityState::NotOffered;
    let problem = entry
        .uses
        .iter()
        .find(|use_| use_.capability == capability)
        .and_then(|use_| use_.problem);
    let note = if not_offered {
        Some(l10n::settings_account_use_not_offered())
    } else if let Some(problem) = problem {
        Some(problem_note(capability, problem))
    } else if last {
        Some(l10n::settings_account_use_last())
    } else if without_contacts {
        Some(l10n::settings_account_colleagues_needs_contacts())
    } else if without_mail {
        Some(l10n::settings_account_shared_needs_mail())
    } else if state == CapabilityState::NeedsPermission {
        Some(l10n::settings_account_use_withheld())
    } else {
        None
    };
    UseSwitch {
        active,
        asks: state == CapabilityState::NeedsPermission,
        sensitive: !last && !without_contacts && !without_mail && !not_offered,
        note,
    }
}

/// One link slot's picker: "none" first, then the accounts it may name.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::ui) struct LinkPicker {
    pub(in crate::ui) slot: LinkSlot,
    pub(in crate::ui) title: &'static str,
    pub(in crate::ui) options: Vec<LinkedAccount>,
    /// The index into `options` of the account linked now; `None` for "none".
    pub(in crate::ui) selected: Option<usize>,
    /// The index into `options` of the account to suggest, while nothing is linked: one whose
    /// calendar server schedules as the mail account's address.
    pub(in crate::ui) suggested: Option<usize>,
}

impl LinkPicker {
    /// What the dropdown lists: "none", then each account as a person reads it.
    pub(in crate::ui) fn labels(&self) -> Vec<String> {
        std::iter::once(l10n::settings_account_link_none().to_owned())
            .chain(self.options.iter().map(linked_name))
            .collect()
    }

    /// The suggested account, as a person reads it.
    pub(in crate::ui) fn suggested_name(&self) -> Option<String> {
        self.suggested
            .and_then(|index| self.options.get(index))
            .map(linked_name)
    }
}

/// The pickers an account's page offers: one per slot it can hold, which is a slot with
/// candidates or one already linked.
pub(in crate::ui) fn link_pickers(entry: &AccountEntry) -> Vec<LinkPicker> {
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
