//! What Settings → Accounts says about an account, decided from its snapshot entry alone, so the
//! rules are tested without a window.

use mailcal_bindings::{
    AccountCapability, AccountChangeNotice, AccountEntry, AccountKind, AccountLinkPicker,
    AccountUseBlock, AccountUseStatus, AccountUseSwitch, CapabilityState, LinkSlot, UseProblem,
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
        AccountCapability::Meetings => l10n::account_use_meetings(),
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
        UseProblem::NoneOnServer if calendar => {
            l10n::settings_account_use_none_on_server_calendar()
        }
        UseProblem::NoneOnServer => l10n::settings_account_use_none_on_server_contacts(),
    }
}

/// What a use's switch says beneath it: what is wrong with the use, then why it cannot be
/// pressed, so neither hides the other.
pub(super) fn switch_note(switch: &AccountUseSwitch) -> Option<String> {
    let status = switch.status.map(|status| match status {
        AccountUseStatus::Problem { problem } => problem_note(switch.capability, problem),
        AccountUseStatus::Withheld => l10n::settings_account_use_withheld(),
    });
    let blocked = switch.blocked.map(|block| match block {
        AccountUseBlock::LastUse => l10n::settings_account_use_last(),
        AccountUseBlock::NeedsContacts => l10n::settings_account_colleagues_needs_contacts(),
        AccountUseBlock::NeedsMail => l10n::settings_account_shared_needs_mail(),
        AccountUseBlock::NotOffered => l10n::settings_account_use_not_offered(),
        AccountUseBlock::NeedsServer => l10n::settings_account_use_needs_endpoint(),
    });
    let lines = [status, blocked].into_iter().flatten().collect::<Vec<_>>();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// What a change on an account's page came to, when the core says it needs saying.
pub(in crate::ui) fn change_notice(notice: &AccountChangeNotice) -> super::notice::Notice {
    match notice {
        AccountChangeNotice::UseFailed { detail, .. }
        | AccountChangeNotice::LinkFailed { detail, .. } => super::notice::Notice::Error {
            title: l10n::settings_account_change_failed_title().to_owned(),
            detail: detail.clone(),
        },
        AccountChangeNotice::RemoveFailed { detail } => super::notice::Notice::Error {
            title: l10n::settings_account_change_failed_title().to_owned(),
            detail: l10n::remove_account_failed(detail),
        },
    }
}

/// A link slot's picker's title.
pub(in crate::ui) fn link_title(slot: LinkSlot) -> &'static str {
    match slot {
        LinkSlot::Calendar => l10n::settings_account_link_calendar(),
        LinkSlot::Contacts => l10n::settings_account_link_contacts(),
        LinkSlot::Mail => l10n::settings_account_link_mail(),
    }
}

/// What a link picker lists: "none", then each account as a person reads it.
pub(in crate::ui) fn link_labels(picker: &AccountLinkPicker) -> Vec<String> {
    std::iter::once(l10n::settings_account_link_none().to_owned())
        .chain(picker.options.iter().map(linked_name))
        .collect()
}

/// The account a link picker suggests, as a person reads it.
pub(in crate::ui) fn suggested_name(picker: &AccountLinkPicker) -> Option<String> {
    picker
        .suggested
        .and_then(|index| picker.options.get(usize::try_from(index).ok()?))
        .map(linked_name)
}

#[cfg(test)]
#[path = "account_rows_tests.rs"]
mod tests;
