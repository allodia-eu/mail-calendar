//! An account's page in Settings (`docs/accounts.md` rules 11 to 13), drawn from the account's
//! entry, the change running and what pressing each switch would come to, which the registry's
//! own `toggle` answers: the check that refuses a change and the page that predicts it are one
//! decision. Every client draws the [`AccountPage`] this makes and decides none of it.

use crate::{
    AccountCapability, AccountChangeNoticeRaised, AccountEndpoints, AccountEntry,
    AccountLinkPicker, AccountPage, AccountReplyFallback, AccountServers, AccountUseStatus,
    AccountUseSwitch, CapabilityState, ControlState, LinkSlot, ReplyFallbackChoice, UseProblem,
    account_registry::Toggle,
    account_work::{Hold, PageWork},
    app_accounts_uses::domain_of,
};

/// `entry`'s page. `hold` says which change runs, `stored_servers` are its servers and login for
/// an account whose page edits them, `refused_servers` is the edit its last save of servers
/// refused, `reply_fallback` the account's standing answer about invitation replies, and
/// `toggle` what switching a use on (`true`) or off comes to. The control a running change started
/// is drawn at the value it is moving to.
pub(crate) fn page(
    entry: &AccountEntry,
    hold: Hold,
    notice: Option<AccountChangeNoticeRaised>,
    stored_servers: Option<AccountEndpoints>,
    refused_servers: Option<AccountEndpoints>,
    reply_fallback: ReplyFallbackChoice,
    toggle: impl Fn(AccountCapability, bool) -> Toggle,
) -> AccountPage {
    let control = |running: bool, free: bool| ControlState {
        enabled: free && !hold.busy,
        running,
    };
    let review = review_after(entry);
    let uses = entry
        .uses
        .iter()
        .map(|use_| {
            let capability = use_.capability;
            let on = use_.state == CapabilityState::On;
            let going = hold.going(|work| match work {
                PageWork::Use { capability: at, on } if *at == capability => Some(*on),
                _ => None,
            });
            let pressing = toggle(capability, !on);
            let blocked = match pressing {
                Toggle::Refused(block) => Some(block),
                _ => None,
            };
            AccountUseSwitch {
                capability,
                on: going.unwrap_or(on),
                control: control(going.is_some(), blocked.is_none()),
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
                signs_in_separately: false,
            }
        })
        .collect();
    let linking = hold.going(|work| match work {
        PageWork::Link { slot, target } => Some((*slot, target.as_deref())),
        _ => None,
    });
    AccountPage {
        account: entry.id.clone(),
        uses,
        links: link_pickers(entry)
            .into_iter()
            .map(|mut picker| {
                if let Some((_, target)) = linking.filter(|(slot, _)| *slot == picker.slot) {
                    picker.selected = target.and_then(|target| {
                        index(picker.options.iter().position(|option| option.id == target))
                    });
                    picker.control = control(true, true);
                } else {
                    picker.control = control(false, true);
                }
                picker
            })
            .collect(),
        remove: control(hold.own.contains(&PageWork::Remove), true),
        sender_name: None,
        mail: None,
        servers: stored_servers.map(|stored| {
            let saving = hold.going(|work| match work {
                PageWork::Servers(edit) => Some(edit.as_ref().clone()),
                _ => None,
            });
            AccountServers {
                sign_in_refused: false,
                save: control(saving.is_some(), true),
                form: saving.or(refused_servers).unwrap_or(stored),
            }
        }),
        shared: None,
        sign_in: None,
        separate_server: None,
        reply_fallback: entry
            .uses
            .iter()
            .any(|use_| {
                use_.capability == AccountCapability::Calendar && use_.state == CapabilityState::On
            })
            .then(|| {
                let going = hold.going(|work| match work {
                    PageWork::ReplyFallback { choice } => Some(*choice),
                    _ => None,
                });
                AccountReplyFallback {
                    choice: going.unwrap_or(reply_fallback),
                    control: control(going.is_some(), true),
                }
            }),
        notice,
    }
}

/// The pickers setup's link step offers `entry` once it is added: one per slot it can hold that
/// nothing fills yet (`docs/accounts.md` rule 12). Whether the step is offered at all is
/// [`offers_setup_links`](crate::offers_setup_links).
#[uniffi::export]
#[must_use]
pub fn setup_link_pickers(entry: &AccountEntry) -> Vec<AccountLinkPicker> {
    link_pickers(entry)
        .into_iter()
        .filter(|picker| picker.selected.is_none())
        .collect()
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

/// A position in a picker's options, as the record carries it.
fn index(found: Option<usize>) -> Option<u32> {
    found.and_then(|at| u32::try_from(at).ok())
}

#[cfg(test)]
#[path = "account_page_tests.rs"]
mod tests;

#[path = "account_page_shared.rs"]
pub(crate) mod shared;

#[path = "account_page_sign_in.rs"]
pub(crate) mod sign_in;

#[path = "account_page_separate.rs"]
pub(crate) mod separate;

#[path = "account_page_answers.rs"]
pub(crate) mod answers;
