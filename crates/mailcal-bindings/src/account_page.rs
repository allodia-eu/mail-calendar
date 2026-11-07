//! An account's page in Settings (`docs/accounts.md` rules 11 to 13), drawn from the account's
//! entry, the change running and what pressing each switch would come to, which the registry's
//! own `toggle` answers: the check that refuses a change and the page that predicts it are one
//! decision. Every client draws the [`AccountPage`] this makes and decides none of it.

use crate::{
    AccountCapability, AccountChangeNoticeRaised, AccountEntry, AccountLinkPicker, AccountPage,
    AccountUseStatus, AccountUseSwitch, CapabilityState, ControlState, LinkSlot, UseProblem,
    account_registry::Toggle,
    account_work::{Hold, PageWork},
    app_accounts_uses::domain_of,
};

/// `entry`'s page. `hold` says which change runs, and `toggle` what switching a use on (`true`)
/// or off comes to.
pub(crate) fn page(
    entry: &AccountEntry,
    hold: Hold,
    notice: Option<AccountChangeNoticeRaised>,
    toggle: impl Fn(AccountCapability, bool) -> Toggle,
) -> AccountPage {
    let control = |work: PageWork, free: bool| ControlState {
        enabled: free && !hold.busy,
        running: hold.own == Some(work),
    };
    let review = review_after(entry);
    let uses = entry
        .uses
        .iter()
        .map(|use_| {
            let capability = use_.capability;
            let on = use_.state == CapabilityState::On;
            let pressing = toggle(capability, !on);
            let blocked = match pressing {
                Toggle::Refused(block) => Some(block),
                _ => None,
            };
            AccountUseSwitch {
                capability,
                on,
                control: control(PageWork::Use(capability), blocked.is_none()),
                blocked,
                status: use_
                    .problem
                    .map(|problem| AccountUseStatus::Problem { problem })
                    .or((use_.state == CapabilityState::NeedsPermission)
                        .then_some(AccountUseStatus::Withheld)),
                asks_provider: pressing == Toggle::AsksProvider,
                // Switching off asks first exactly when it deletes something.
                confirms_off: domain_of(capability.into()).is_some(),
                reviews_server_change: review == Some(capability),
            }
        })
        .collect();
    let links = link_pickers(entry)
        .into_iter()
        .map(|mut picker| {
            picker.control = control(PageWork::Link(picker.slot), true);
            picker
        })
        .collect();
    AccountPage {
        account: entry.id.clone(),
        uses,
        links,
        remove: control(PageWork::Remove, true),
        notice,
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
