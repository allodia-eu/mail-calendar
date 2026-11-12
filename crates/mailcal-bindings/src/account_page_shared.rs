//! An account's shared stores on its page (`docs/shared-mailboxes.md` rules 3, 7 and 8): what
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

use mailcal_account::SharedStoreChoice;
use mailcal_app::SharedStoreOffer;

use crate::{
    AccountSharedAddress, AccountSharedStore, AccountSharedStores, ControlState, SharedStoreHolds,
    SharedStoreSupport, SharedStoresNote,
    account_work::{Hold, PageWork},
    app_shared_stores::view,
};

/// What a server answered: its offer, or `Err` when it was asked and did not answer.
pub(crate) type Offer = Result<SharedStoreOffer, ()>;

/// The account as its server was asked: what it is used for, and which of its mailbox and
/// calendar were live. An answer asked of an account that has changed since may no longer hold: a
/// use switched on or off opens or closes them, and a mailbox or calendar that connected can now
/// be asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Asked {
    pub(crate) uses: mailcal_account::Capabilities,
    pub(crate) live: mailcal_account::SharedUses,
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
        // An account forgotten while its server was asked is gone: nothing is kept for it.
        let Some(answer) = answers.get_mut(account) else {
            return;
        };
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

    pub(crate) fn missed_at(&self, account: &str) -> Option<String> {
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
        chosen: Vec<SharedStoreChoice>,
        hold: &Hold,
    ) -> Option<AccountSharedStores> {
        section(offer?, chosen, hold, self.missed_at(account))
    }
}

fn section(
    offer: Offer,
    chosen: Vec<SharedStoreChoice>,
    hold: &Hold,
    missed: Option<String>,
) -> Option<AccountSharedStores> {
    let view = view(offer, chosen);
    if view.support == SharedStoreSupport::Unsupported && view.stores.is_empty() {
        return None;
    }
    let control = |running: bool| ControlState {
        enabled: !hold.busy,
        running,
    };
    let stores = view
        .stores
        .iter()
        // One holding nothing the account opens would switch on to nothing.
        .filter_map(|store| Some((store, holds(store.holds)?)))
        .map(|(store, holds)| {
            let going = hold.going(|work| match work {
                PageWork::Shared { handle, on } if *handle == store.handle => Some(*on),
                _ => None,
            });
            let title = store.address.clone().unwrap_or_else(|| store.name.clone());
            AccountSharedStore {
                handle: store.handle.clone(),
                label: (store.name != title).then(|| store.name.clone()),
                title,
                holds,
                withdrawn: !store.listed,
                on: going.unwrap_or(store.selected),
                control: control(going.is_some()),
            }
        })
        .collect::<Vec<_>>();
    let note = match view.support {
        SharedStoreSupport::Listed if stores.is_empty() => Some(SharedStoresNote::NoneShared),
        SharedStoreSupport::ByAddress => Some(SharedStoresNote::ByAddress),
        SharedStoreSupport::NotConnected => Some(SharedStoresNote::NotConnected),
        SharedStoreSupport::Unreachable => Some(SharedStoresNote::Unreachable),
        _ => None,
    };
    let address = (view.support == SharedStoreSupport::ByAddress).then(|| {
        let adding = hold.going(|work| match work {
            PageWork::AddShared(address) => Some(address.clone()),
            _ => None,
        });
        AccountSharedAddress {
            add: control(adding.is_some()),
            text: adding.or(missed).unwrap_or_default(),
        }
    });
    Some(AccountSharedStores {
        stores,
        note,
        address,
    })
}

/// What a client says a store holds; `None` for nothing.
fn holds(uses: mailcal_account::SharedUses) -> Option<SharedStoreHolds> {
    use mailcal_account::SharedUse::{Calendar, Mail};
    match (uses.contains(Mail), uses.contains(Calendar)) {
        (true, true) => Some(SharedStoreHolds::MailAndCalendar),
        (true, false) => Some(SharedStoreHolds::Mail),
        (false, true) => Some(SharedStoreHolds::Calendar),
        (false, false) => None,
    }
}

#[cfg(test)]
#[path = "account_page_shared_tests.rs"]
mod tests;
