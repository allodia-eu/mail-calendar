//! What Settings → Accounts says about an account, decided from its snapshot entry alone, so the
//! rules are tested without a window.

use mailcal_bindings::{
    AccountCapability, AccountChangeNotice, AccountEntry, AccountKind, AccountLinkPicker,
    AccountLinkedFrom, AccountUseBlock, AccountUseStatus, AccountUseSwitch, CapabilityState,
    LinkSlot, LinkedAccount, UseProblem,
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
    let blocked = switch.blocked.map(block_note);
    let lines = [status, blocked].into_iter().flatten().collect::<Vec<_>>();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// Why a use's switch cannot be pressed.
pub(in crate::ui) fn block_note(block: AccountUseBlock) -> &'static str {
    match block {
        AccountUseBlock::LastUse => l10n::settings_account_use_last(),
        AccountUseBlock::NeedsContacts => l10n::settings_account_colleagues_needs_contacts(),
        AccountUseBlock::NeedsMailOrCalendar => l10n::settings_account_shared_needs_mail(),
        AccountUseBlock::NotOffered => l10n::settings_account_use_not_offered(),
        AccountUseBlock::NeedsServer => l10n::settings_account_use_needs_endpoint(),
    }
}

/// What a change on an account's page came to, when the core says it needs saying.
pub(in crate::ui) fn change_notice(notice: &AccountChangeNotice) -> super::notice::Notice {
    match notice {
        AccountChangeNotice::UseFailed { refusal, .. }
        | AccountChangeNotice::LinkFailed { refusal, .. }
        | AccountChangeNotice::SharedStoreFailed { refusal }
        | AccountChangeNotice::PasswordFailed { refusal }
        | AccountChangeNotice::StopLinkingFailed { refusal } => super::notice::Notice::Error {
            title: l10n::settings_account_change_failed_title().to_owned(),
            detail: crate::ui::core_error::refusal_said(refusal),
        },
        AccountChangeNotice::RemoveFailed { detail } => super::notice::Notice::Error {
            title: l10n::settings_account_change_failed_title().to_owned(),
            detail: l10n::remove_account_failed(detail),
        },
        AccountChangeNotice::ServersFailed { refusal } => super::notice::Notice::Error {
            title: l10n::settings_account_servers_failed().to_owned(),
            detail: crate::ui::core_error::refusal_said(refusal),
        },
        AccountChangeNotice::SignInFailed { switching, refusal } => super::notice::Notice::Error {
            title: if *switching {
                l10n::settings_account_switch_failed()
            } else {
                l10n::settings_account_signin_failed()
            }
            .to_owned(),
            detail: crate::ui::core_error::refusal_said(refusal),
        },
        AccountChangeNotice::SignedInWithheld { uses } => {
            crate::ui::oauth_actions::withheld_said(uses)
        }
        AccountChangeNotice::SignedInSeparately { uses } => super::notice::Notice::Explained {
            title: l10n::settings_account_signed_in_separately_title().to_owned(),
            detail: by_uses(
                uses,
                l10n::settings_account_signed_in_separately_calendar(),
                l10n::settings_account_signed_in_separately_contacts(),
                l10n::settings_account_signed_in_separately_both(),
            )
            .to_owned(),
        },
        // Not a failure of the app: the address is the person's to check.
        AccountChangeNotice::SharedStoreNotFound { .. } => {
            super::notice::Notice::Toast(l10n::settings_account_shared_not_found().to_owned())
        }
    }
}

/// One of three wordings, as `uses` holds the calendar, the address book or both.
fn by_uses<'a>(
    uses: &[AccountCapability],
    calendar: &'a str,
    contacts: &'a str,
    both: &'a str,
) -> &'a str {
    match (
        uses.contains(&AccountCapability::Calendar),
        uses.contains(&AccountCapability::Contacts),
    ) {
        (true, true) => both,
        (false, true) => contacts,
        _ => calendar,
    }
}

/// Beneath a use a link fills: the account it comes from.
pub(in crate::ui) fn linked_note(linked: &LinkedAccount) -> String {
    l10n::settings_account_use_linked(&linked_name(linked))
}

/// Beneath an account taking a use from this one: what it takes.
pub(in crate::ui) fn linked_from_line(linked: &AccountLinkedFrom) -> &'static str {
    by_uses(
        &linked.uses,
        l10n::settings_account_linked_from_calendar(),
        l10n::settings_account_linked_from_contacts(),
        l10n::settings_account_linked_from_both(),
    )
}

/// What stopping the links with `linked` does, said before it is confirmed.
pub(in crate::ui) fn stop_linking_message(linked: &AccountLinkedFrom) -> String {
    let mut message = by_uses(
        &linked.uses,
        l10n::settings_account_stop_linking_calendar(),
        l10n::settings_account_stop_linking_contacts(),
        l10n::settings_account_stop_linking_both(),
    )
    .to_owned();
    if !linked.leaves_without.is_empty() {
        message.push(' ');
        message.push_str(l10n::settings_account_stop_linking_left_without());
    }
    message
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
