//! Building Settings → Accounts from what each account stores, and deciding which of its stored
//! links still hold.

use std::collections::BTreeSet;

use engine_api::AccountId;
use mailcal_account::{AccountLinks, Capabilities, Capability};

use crate::{
    AccountEntry, AccountKind, AccountLinksView, AccountUse, CapabilityState, LinkedAccount,
};

/// What the snapshot needs to know about one account, read from its stored config.
#[derive(Debug, Clone)]
pub(crate) struct AccountFacts {
    pub(crate) id: String,
    pub(crate) address: String,
    pub(crate) kind: AccountKind,
    /// What the account is used for.
    pub(crate) chosen: Capabilities,
    /// What its Microsoft or Google grant does not allow of that.
    pub(crate) withheld: Capabilities,
    /// Whether its calendar is a CalDAV one, which is what an invitation that arrived by mail
    /// can be filed into.
    pub(crate) files_invitations: bool,
    /// Its links as stored, valid or not.
    pub(crate) links: AccountLinks,
}

/// Every account's entry, in the order `facts` lists them. `calendar_refused` holds the accounts
/// whose provider refused their calendar when it was opened.
pub(crate) fn entries(
    facts: &[AccountFacts],
    calendar_refused: &BTreeSet<String>,
) -> Vec<AccountEntry> {
    let links = valid_links(facts);
    let named = |target: Option<&AccountId>| {
        target.and_then(|target| {
            facts
                .iter()
                .find(|account| account.id == target.as_str())
                .map(linked)
        })
    };
    facts
        .iter()
        .zip(&links)
        .map(|(account, own)| AccountEntry {
            id: account.id.clone(),
            address: account.address.clone(),
            kind: account.kind,
            uses: uses(account, calendar_refused.contains(&account.id)),
            links: AccountLinksView {
                calendar: named(own.calendar.as_ref()),
                contacts: named(own.contacts.as_ref()),
                mail: named(own.mail.as_ref()),
            },
            linked_from: facts
                .iter()
                .zip(&links)
                .filter(|(_, theirs)| {
                    [&theirs.calendar, &theirs.contacts, &theirs.mail]
                        .into_iter()
                        .flatten()
                        .any(|target| target.as_str() == account.id)
                })
                .map(|(other, _)| linked(other))
                .collect(),
        })
        .collect()
}

/// The links each account holds that the accounts allow, in the order `facts` lists them.
///
/// A link fills a use the account is not used for itself, and names an account used for it; a
/// calendar link names a CalDAV calendar. A calendar account's mail link names one of the mail
/// accounts whose calendar link is it, and is implied when there is only one. Anything else reads
/// as no link, so a link left behind by a removal or a change of use is never acted on.
pub(crate) fn valid_links(facts: &[AccountFacts]) -> Vec<AccountLinks> {
    let target = |from: &AccountFacts, to: Option<&AccountId>, capability: Capability| {
        let to = to?;
        let found = facts.iter().find(|account| account.id == to.as_str())?;
        let allowed = found.id != from.id
            && !from.chosen.contains(capability)
            && found.chosen.contains(capability)
            && (capability != Capability::Calendar || found.files_invitations);
        allowed.then(|| to.clone())
    };
    let filled: Vec<AccountLinks> = facts
        .iter()
        .map(|account| AccountLinks {
            calendar: target(
                account,
                account.links.calendar.as_ref(),
                Capability::Calendar,
            ),
            contacts: target(
                account,
                account.links.contacts.as_ref(),
                Capability::Contacts,
            ),
            mail: None,
        })
        .collect();
    facts
        .iter()
        .zip(&filled)
        .map(|(account, own)| {
            let senders: Vec<&AccountFacts> = if account.chosen.contains(Capability::Mail) {
                Vec::new()
            } else {
                facts
                    .iter()
                    .zip(&filled)
                    .filter(|(other, theirs)| {
                        other.chosen.contains(Capability::Mail)
                            && theirs
                                .calendar
                                .as_ref()
                                .is_some_and(|calendar| calendar.as_str() == account.id)
                    })
                    .map(|(other, _)| other)
                    .collect()
            };
            let named = account
                .links
                .mail
                .as_ref()
                .filter(|mail| senders.iter().any(|sender| sender.id == mail.as_str()));
            let mail = match (named, senders.as_slice()) {
                (Some(mail), _) => Some(mail.clone()),
                (None, [only]) => AccountId::try_from(only.id.as_str()).ok(),
                (None, _) => None,
            };
            AccountLinks {
                mail,
                ..own.clone()
            }
        })
        .collect()
}

/// The uses `account`'s kind can offer, each in its state.
fn uses(account: &AccountFacts, calendar_refused: bool) -> Vec<AccountUse> {
    let offered: &[Capability] = match account.kind {
        AccountKind::Microsoft | AccountKind::Google => &Capability::ALL,
        AccountKind::Imap | AccountKind::Dav | AccountKind::Jmap => {
            &[Capability::Mail, Capability::Calendar, Capability::Contacts]
        }
    };
    offered
        .iter()
        .map(|&capability| {
            let refused = account.withheld.contains(capability)
                || (capability == Capability::Calendar && calendar_refused);
            let state = match (account.chosen.contains(capability), refused) {
                (false, _) => CapabilityState::Off,
                (true, false) => CapabilityState::On,
                (true, true) => CapabilityState::NeedsPermission,
            };
            AccountUse {
                capability: capability.into(),
                state,
            }
        })
        .collect()
}

fn linked(account: &AccountFacts) -> LinkedAccount {
    LinkedAccount {
        id: account.id.clone(),
        address: account.address.clone(),
    }
}

#[cfg(test)]
#[path = "accounts_view_tests.rs"]
mod tests;
