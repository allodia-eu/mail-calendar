//! An account's shared mailboxes on its page (`docs/shared-mailboxes.md` rules 3, 7 and 8): what
//! its server answered, kept so the page is drawn on every signal without asking the server each
//! time, and asked again when the page opens or the account changed since it was asked. The rows,
//! the line under them and the address field are decided here, so no client keeps a copy.

use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use mailcal_account::SharedMailboxChoice;
use mailcal_app::SharedMailboxOffer;

use crate::{
    AccountSharedAddress, AccountSharedMailbox, AccountSharedMailboxes, ControlState,
    SharedMailboxSupport, SharedMailboxesNote,
    account_work::{Hold, PageWork},
    app_shared_mailboxes::view,
};

/// What a server answered: its offer, or `Err` when it was asked and did not answer.
pub(crate) type Offer = Result<SharedMailboxOffer, ()>;

/// The account as its server was asked: what it is used for, and whether its mailbox was live.
/// An answer asked of an account that has changed since may no longer hold: a use switched on or
/// off opens or closes them, and a mailbox that connected can now be asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Asked {
    pub(crate) uses: String,
    pub(crate) live: bool,
}

#[derive(Debug, Default)]
struct Answer {
    offer: Option<Offer>,
    asked: Option<Asked>,
    /// The ask the kept offer came from, so an older ask landing later is dropped.
    answered: u64,
    /// The ask out now, if any: one at a time.
    asking: Option<u64>,
    /// The address the last add found nothing at, for the field to be drawn with.
    missed: Option<String>,
}

/// Every account's answer.
#[derive(Debug, Default)]
pub(crate) struct SharedAnswers {
    answers: Mutex<HashMap<String, Answer>>,
    tickets: AtomicU64,
}

impl SharedAnswers {
    fn answers(&self) -> std::sync::MutexGuard<'_, HashMap<String, Answer>> {
        self.answers.lock().expect("shared answers mutex poisoned")
    }

    /// A ticket to ask `account`'s server now, the account being `now`: when it has not answered,
    /// when asked `again` (its page opened), or when the account changed since it was asked.
    /// `None` while an ask is out, whose answer is the one to wait for.
    pub(crate) fn ask(&self, account: &str, now: &Asked, again: bool) -> Option<u64> {
        let mut answers = self.answers();
        let answer = answers.entry(account.to_owned()).or_default();
        if answer.asking.is_some()
            || !(again || answer.offer.is_none() || answer.asked.as_ref() != Some(now))
        {
            return None;
        }
        let ticket = self.tickets.fetch_add(1, Ordering::Relaxed) + 1;
        answer.asking = Some(ticket);
        answer.asked = Some(now.clone());
        Some(ticket)
    }

    /// The server answered the ask `ticket` with `offer`.
    pub(crate) fn answered(&self, account: &str, ticket: u64, offer: Offer) {
        let mut answers = self.answers();
        let answer = answers.entry(account.to_owned()).or_default();
        if answer.asking == Some(ticket) {
            answer.asking = None;
        }
        if ticket > answer.answered {
            answer.offer = Some(offer);
            answer.answered = ticket;
        }
    }

    /// What `account`'s server last answered.
    pub(crate) fn offer(&self, account: &str) -> Option<Offer> {
        self.answers()
            .get(account)
            .and_then(|answer| answer.offer.clone())
    }

    /// What `account`'s last add found nothing at, or `None` once one was found.
    pub(crate) fn missed(&self, account: &str, address: Option<String>) {
        self.answers().entry(account.to_owned()).or_default().missed = address;
    }

    fn missed_at(&self, account: &str) -> Option<String> {
        self.answers()
            .get(account)
            .and_then(|answer| answer.missed.clone())
    }

    /// Drops what is kept for `account`, which is gone.
    pub(crate) fn forget(&self, account: &str) {
        self.answers().remove(account);
    }

    /// `account`'s section, from `offer` (`None` before its server answered) and the mailboxes it
    /// is used for, held as `hold` says. `None` before the first answer, and for an account that
    /// cannot open one and has none chosen.
    pub(crate) fn section(
        &self,
        account: &str,
        offer: Option<Offer>,
        chosen: Vec<SharedMailboxChoice>,
        hold: &Hold,
    ) -> Option<AccountSharedMailboxes> {
        section(offer?, chosen, hold, self.missed_at(account))
    }
}

fn section(
    offer: Offer,
    chosen: Vec<SharedMailboxChoice>,
    hold: &Hold,
    missed: Option<String>,
) -> Option<AccountSharedMailboxes> {
    let view = view(offer, chosen);
    if view.support == SharedMailboxSupport::Unsupported && view.mailboxes.is_empty() {
        return None;
    }
    let control = |running: bool| ControlState {
        enabled: !hold.busy,
        running,
    };
    let mailboxes = view
        .mailboxes
        .iter()
        .map(|mailbox| {
            let going = hold.going(|work| match work {
                PageWork::Shared { handle, on } if *handle == mailbox.handle => Some(*on),
                _ => None,
            });
            let title = mailbox
                .address
                .clone()
                .unwrap_or_else(|| mailbox.name.clone());
            AccountSharedMailbox {
                handle: mailbox.handle.clone(),
                label: (mailbox.name != title).then(|| mailbox.name.clone()),
                title,
                withdrawn: !mailbox.listed,
                on: going.unwrap_or(mailbox.selected),
                control: control(going.is_some()),
            }
        })
        .collect::<Vec<_>>();
    let note = match view.support {
        SharedMailboxSupport::Listed if mailboxes.is_empty() => {
            Some(SharedMailboxesNote::NoneShared)
        }
        SharedMailboxSupport::ByAddress => Some(SharedMailboxesNote::ByAddress),
        SharedMailboxSupport::NotConnected => Some(SharedMailboxesNote::NotConnected),
        SharedMailboxSupport::Unreachable => Some(SharedMailboxesNote::Unreachable),
        _ => None,
    };
    let address = (view.support == SharedMailboxSupport::ByAddress).then(|| {
        let adding = hold.going(|work| match work {
            PageWork::AddShared(address) => Some(address.clone()),
            _ => None,
        });
        AccountSharedAddress {
            add: control(adding.is_some()),
            text: adding.or(missed).unwrap_or_default(),
        }
    });
    Some(AccountSharedMailboxes {
        mailboxes,
        note,
        address,
    })
}

#[cfg(test)]
#[path = "account_page_shared_tests.rs"]
mod tests;
