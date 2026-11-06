//! The rules of an account's page in Settings (`docs/accounts.md` rules 11 to 13), decided once
//! from the account's entry, the work its page has running and what switching a use on needs.
//! Every client draws the [`AccountPage`] this makes and decides none of it.

use crate::{
    AccountCapability, AccountChangeNoticeRaised, AccountEntry, AccountLinkPicker, AccountPage,
    AccountUseNote, AccountUseSwitch, CapabilityState, ControlState, LinkSlot, UseProblem,
    account_work::{Hold, PageWork},
};

/// What switching an unused use on needs, as the account's registry answers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwitchOn {
    /// The core switches it on.
    Applies,
    /// The provider has not granted it: its sign-in is asked for it.
    AsksProvider,
    /// The account has no server for it yet.
    NeedsServer,
}

/// The three uses an account is used for at least one of.
const PRIMARY: [AccountCapability; 3] = [
    AccountCapability::Mail,
    AccountCapability::Calendar,
    AccountCapability::Contacts,
];

/// Whether a use in `state` is one the account is used for: on, or waiting on the provider.
const fn works(state: CapabilityState) -> bool {
    matches!(
        state,
        CapabilityState::On | CapabilityState::NeedsPermission
    )
}

/// `entry`'s page, with `hold` holding its controls and `switch_on` answering what turning an
/// unused use on needs.
pub(crate) fn page(
    entry: &AccountEntry,
    hold: Hold,
    notice: Option<AccountChangeNoticeRaised>,
    switch_on: impl Fn(AccountCapability) -> SwitchOn,
) -> AccountPage {
    let control = |work: PageWork| ControlState {
        enabled: !hold.busy,
        running: hold.own == Some(work),
    };
    let review = review_after(entry);
    let uses = entry
        .uses
        .iter()
        .map(|use_| {
            let mut switch = use_switch(entry, use_.capability, &switch_on);
            switch.control.enabled &= !hold.busy;
            switch.control.running = hold.own == Some(PageWork::Use(use_.capability));
            switch.reviews_server_change = review == Some(use_.capability);
            switch
        })
        .collect();
    let links = link_pickers(entry)
        .into_iter()
        .map(|mut picker| {
            picker.control = control(PageWork::Link(picker.slot));
            picker
        })
        .collect();
    AccountPage {
        account: entry.id.clone(),
        uses,
        links,
        remove: control(PageWork::Remove),
        notice,
    }
}

/// The switch for `capability`. The last of mail, calendar and contacts cannot be switched off,
/// colleagues cannot be switched on without contacts, nor shared mailboxes without mail, and a use
/// with no server cannot be switched on until it has one: the core refuses each, so the switch
/// says why rather than letting the person find out.
fn use_switch(
    entry: &AccountEntry,
    capability: AccountCapability,
    switch_on: impl Fn(AccountCapability) -> SwitchOn,
) -> AccountUseSwitch {
    let used = |wanted| {
        entry
            .uses
            .iter()
            .any(|use_| use_.capability == wanted && works(use_.state))
    };
    let this = entry.uses.iter().find(|use_| use_.capability == capability);
    let state = this.map_or(CapabilityState::Off, |use_| use_.state);
    let problem = this.and_then(|use_| use_.problem);
    // A use waiting on a permission is drawn off, so it is never the one held on: switching it
    // on asks the provider, which the core does not refuse.
    let last = state == CapabilityState::On
        && PRIMARY.contains(&capability)
        && PRIMARY.iter().filter(|wanted| used(**wanted)).count() == 1;
    let without = |sub, within| capability == sub && state == CapabilityState::Off && !used(within);
    let without_contacts = without(AccountCapability::Colleagues, AccountCapability::Contacts);
    let without_mail = without(AccountCapability::SharedMailboxes, AccountCapability::Mail);
    let not_offered = state == CapabilityState::NotOffered;
    let needs = (state == CapabilityState::Off).then(|| switch_on(capability));
    let needs_server = needs == Some(SwitchOn::NeedsServer);
    let note = if not_offered {
        Some(AccountUseNote::NotOffered)
    } else if let Some(problem) = problem {
        Some(problem_note(capability, problem))
    } else if last {
        Some(AccountUseNote::Last)
    } else if without_contacts {
        Some(AccountUseNote::NeedsContacts)
    } else if without_mail {
        Some(AccountUseNote::NeedsMail)
    } else if state == CapabilityState::NeedsPermission {
        Some(AccountUseNote::Withheld)
    } else if needs_server {
        Some(AccountUseNote::NeedsServer)
    } else {
        None
    };
    AccountUseSwitch {
        capability,
        on: state == CapabilityState::On,
        control: ControlState {
            enabled: !last && !without_contacts && !without_mail && !not_offered && !needs_server,
            running: false,
        },
        note,
        asks_provider: state == CapabilityState::NeedsPermission
            || needs == Some(SwitchOn::AsksProvider),
        // Video meetings hold nothing on the device to remove (`docs/accounts.md` rule 19).
        confirms_off: capability != AccountCapability::Meetings,
        reviews_server_change: false,
    }
}

/// What a use that did not connect says beneath its switch. Only a calendar or an address book
/// fails on its own; mail decides whether the account connected at all.
const fn problem_note(capability: AccountCapability, problem: UseProblem) -> AccountUseNote {
    let calendar = matches!(capability, AccountCapability::Calendar);
    match problem {
        UseProblem::Unreachable if calendar => AccountUseNote::UnreachableCalendar,
        UseProblem::Unreachable => AccountUseNote::UnreachableContacts,
        UseProblem::SignInRefused if calendar => AccountUseNote::RefusedCalendar,
        UseProblem::SignInRefused => AccountUseNote::RefusedContacts,
        UseProblem::NeedsOwnSignIn if calendar => AccountUseNote::OwnSignInCalendar,
        UseProblem::NeedsOwnSignIn => AccountUseNote::OwnSignInContacts,
        UseProblem::ServerChanged if calendar => AccountUseNote::ServerChangedCalendar,
        UseProblem::ServerChanged => AccountUseNote::ServerChangedContacts,
        UseProblem::NoneOnServer if calendar => AccountUseNote::NoneOnServerCalendar,
        UseProblem::NoneOnServer => AccountUseNote::NoneOnServerContacts,
    }
}

/// The use the page's "Review" sits under: the last of those paused for a server found on another
/// site, so it follows every note that says so. `None` when none is.
fn review_after(entry: &AccountEntry) -> Option<AccountCapability> {
    entry
        .uses
        .iter()
        .rfind(|use_| use_.problem == Some(UseProblem::ServerChanged))
        .map(|use_| use_.capability)
}

/// The pickers the page offers: one per slot it can hold, which is a slot with candidates or one
/// already linked. The linked account is listed even when it is no longer a candidate. Matched by
/// id, so a linked account whose address or distinction has since changed is not listed twice.
fn link_pickers(entry: &AccountEntry) -> Vec<AccountLinkPicker> {
    [
        (
            LinkSlot::Calendar,
            &entry.link_candidates.calendar,
            &entry.links.calendar,
        ),
        (
            LinkSlot::Contacts,
            &entry.link_candidates.contacts,
            &entry.links.contacts,
        ),
        (
            LinkSlot::Mail,
            &entry.link_candidates.mail,
            &entry.links.mail,
        ),
    ]
    .into_iter()
    .filter(|(_, candidates, current)| !candidates.is_empty() || current.is_some())
    .map(|(slot, candidates, current)| {
        let mut options = candidates.clone();
        if let Some(current) = current
            && !options.iter().any(|option| option.id == current.id)
        {
            options.insert(0, current.clone());
        }
        let index = |found: Option<usize>| found.and_then(|at| u32::try_from(at).ok());
        let selected = index(
            current
                .as_ref()
                .and_then(|current| options.iter().position(|option| option.id == current.id)),
        );
        let suggested = selected
            .is_none()
            .then(|| {
                index(
                    options
                        .iter()
                        .position(|option| entry.link_candidates.suggested.contains(&option.id)),
                )
            })
            .flatten();
        AccountLinkPicker {
            slot,
            options,
            selected,
            suggested,
            control: ControlState {
                enabled: true,
                running: false,
            },
        }
    })
    .collect()
}

#[cfg(test)]
#[path = "account_page_tests.rs"]
mod tests;
