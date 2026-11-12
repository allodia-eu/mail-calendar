//! Asking a connected account which stores its sign-in opens besides its own
//! (`docs/shared-mailboxes.md`). Nothing here is kept: the answer is the server's, read when
//! Settings asks.
//!
//! Each use is asked of the provider that serves it, its mailbox for mail and its calendar for
//! calendars, because each provider reports the stores it can open: an IMAP sign-in's CalDAV half
//! finds calendar homes its IMAP half knows nothing of, and a Microsoft calendar is checked by a
//! calendar probe, not an Inbox one. What they answer is one list: a store both report under one
//! handle, a JMAP account or a Microsoft address, is one store holding both. Each store's
//! [`SharedAccount::domains`] says what it holds of what was asked, which is what switching it on
//! opens.

use engine_api::{
    AccountId, ApiError, FailureClass, Provider, SearchDomain, SharedAccount, SharedAccounts,
    StoreDomains, SyncError,
};
use mailcal_account::{SharedUse, SharedUses};

use crate::App;

/// What an account's sign-in can say about the stores it opens besides its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedStoreOffer {
    /// None of the providers asked is connected, so there is nothing to ask yet.
    NotConnected,
    /// The servers list them.
    Listed(Vec<SharedAccount>),
    /// The servers list none, but answer for an address the person types.
    ByAddress,
    /// The servers have no way to open one.
    Unsupported,
}

/// What the servers said of an address the person typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedStoreLookup {
    /// A store the sign-in opens.
    Found(SharedAccount),
    /// Nothing the sign-in can open answers to it, or it is not an address. The server will not
    /// say which of "no such store" and "not shared with you" it is.
    NotOpenable,
    /// None of the providers asked is connected, so there is nothing to ask yet.
    NotConnected,
}

impl<P: Provider> App<P> {
    /// Which of `uses` `account` has a live provider for, to ask which stores it opens: what
    /// [`App::shared_store_offer`] answers [`SharedStoreOffer::NotConnected`] without.
    pub async fn live_shared_uses(&self, account: &AccountId, uses: SharedUses) -> SharedUses {
        match self.account_handle(account).await {
            Some(handle) => uses
                .iter()
                .filter(|kind| asked_of(&handle, *kind).is_some())
                .collect(),
            None => SharedUses::default(),
        }
    }

    /// Which stores `account`'s sign-in opens besides its own that hold any of `uses`, asked of
    /// its live provider for each.
    ///
    /// A server that lists them outranks one that answers by address, which outranks one that
    /// has no way: the list is what can be drawn, and an address field beside it would offer
    /// what the list already holds.
    ///
    /// # Errors
    ///
    /// Returns the engine's error when a server was asked and did not answer.
    pub async fn shared_store_offer(
        &self,
        account: &AccountId,
        uses: SharedUses,
    ) -> Result<SharedStoreOffer, ApiError> {
        let Some(handle) = self.account_handle(account).await else {
            return Ok(SharedStoreOffer::NotConnected);
        };
        let (mut listed, mut by_address, mut connected) = (None, false, false);
        for kind in uses.iter() {
            let Some(provider) = asked_of(&handle, kind) else {
                continue;
            };
            connected = true;
            match provider.connection_info().capabilities.shared_accounts() {
                SharedAccounts::Unsupported => {}
                SharedAccounts::ByAddress => by_address = true,
                SharedAccounts::Enumerable => {
                    let found = self.engine.list_shared_accounts(provider).await?;
                    let all = listed.get_or_insert_with(Vec::new);
                    for store in found {
                        merge(all, store, kind);
                    }
                }
            }
        }
        Ok(match listed {
            Some(listed) => SharedStoreOffer::Listed(listed),
            None if by_address => SharedStoreOffer::ByAddress,
            None if connected => SharedStoreOffer::Unsupported,
            None => SharedStoreOffer::NotConnected,
        })
    }

    /// The store at `address`, asked of `account`'s live provider for each of `uses`; found when
    /// any of them opens it, holding what each that did found.
    ///
    /// # Errors
    ///
    /// Returns the engine's error when a server did not answer, or refused the sign-in.
    pub async fn resolve_shared_store(
        &self,
        account: &AccountId,
        uses: SharedUses,
        address: &str,
    ) -> Result<SharedStoreLookup, ApiError> {
        let Some(handle) = self.account_handle(account).await else {
            return Ok(SharedStoreLookup::NotConnected);
        };
        let (mut found, mut connected) = (Vec::new(), false);
        for kind in uses.iter() {
            let Some(provider) = asked_of(&handle, kind) else {
                continue;
            };
            connected = true;
            match self.engine.resolve_shared_account(provider, address).await {
                Ok(store) => merge(&mut found, store, kind),
                Err(ApiError::InvalidInput(_)) => {}
                Err(ApiError::Sync(SyncError::Provider(err)))
                    if err.class() == FailureClass::Permanent => {}
                Err(err) => return Err(err),
            }
        }
        // Each provider that opens it answers under the address it was asked, so one handle.
        Ok(match found.into_iter().next() {
            Some(store) => SharedStoreLookup::Found(store),
            None if connected => SharedStoreLookup::NotOpenable,
            None => SharedStoreLookup::NotConnected,
        })
    }
}

/// The provider `account` is asked of for `kind`: its mailbox for mail, its calendar for
/// calendars.
fn asked_of<P>(account: &crate::Account<P>, kind: SharedUse) -> Option<&P> {
    match kind {
        SharedUse::Mail => account.providers.first(),
        SharedUse::Calendar => account.calendar_provider.as_ref(),
    }
}

/// The engine's domain for `kind`.
const fn domain(kind: SharedUse) -> SearchDomain {
    match kind {
        SharedUse::Mail => SearchDomain::Mail,
        SharedUse::Calendar => SearchDomain::Calendar,
    }
}

/// Adds `store`, reported by the provider asked for `kind`, to `into`: `kind` joined to what the
/// store already there under its handle holds, or a store of its own holding `kind` alone. A
/// provider reports whatever it serves of a store, and only `kind` was asked of this one. A store
/// that does not hold `kind` is not one this provider opens for it: a server lists every store the
/// sign-in opens, a colleague's calendar among them, and one without mail is no mailbox.
fn merge(into: &mut Vec<SharedAccount>, mut store: SharedAccount, kind: SharedUse) {
    let asked = StoreDomains::of(domain(kind));
    if !store.domains.contains(domain(kind)) {
        return;
    }
    if let Some(kept) = into.iter_mut().find(|kept| kept.handle == store.handle) {
        kept.domains = kept.domains.union(asked);
    } else {
        store.domains = asked;
        into.push(store);
    }
}

#[cfg(test)]
mod tests {
    use engine_api::{SharedAccountId, StoreDomains};

    use super::*;

    fn holds(store: &SharedAccount) -> SharedUses {
        SharedUse::ALL
            .into_iter()
            .filter(|kind| store.domains.contains(domain(*kind)))
            .collect()
    }

    fn store(handle: &str, domains: StoreDomains) -> SharedAccount {
        SharedAccount::new(SharedAccountId::try_from(handle).unwrap(), handle, domains)
    }

    fn both() -> StoreDomains {
        StoreDomains::of(SearchDomain::Mail).with(SearchDomain::Calendar)
    }

    #[test]
    fn one_handle_reported_for_mail_and_for_calendars_is_one_store_holding_both() {
        let mut offered = Vec::new();
        merge(&mut offered, store("group", both()), SharedUse::Mail);
        merge(&mut offered, store("group", both()), SharedUse::Calendar);
        assert_eq!(offered.len(), 1);
        assert_eq!(
            holds(&offered[0]),
            SharedUses::of(SharedUse::Mail).with(SharedUse::Calendar)
        );
    }

    #[test]
    fn a_store_holds_only_what_the_provider_asked_for_it_found() {
        let mut offered = Vec::new();
        // A JMAP mailbox provider lists a colleague's calendar too; it is no mailbox.
        let colleague = store("colleague", StoreDomains::of(SearchDomain::Calendar));
        merge(&mut offered, colleague.clone(), SharedUse::Mail);
        assert_eq!(offered, []);
        merge(&mut offered, colleague, SharedUse::Calendar);
        assert_eq!(holds(&offered[0]), SharedUses::of(SharedUse::Calendar));
        // A group asked of the mailbox alone holds mail, whatever else it holds.
        merge(&mut offered, store("group", both()), SharedUse::Mail);
        assert_eq!(holds(&offered[1]), SharedUses::of(SharedUse::Mail));
    }

    #[test]
    fn handles_from_different_servers_stay_different_stores() {
        // An IMAP account's mail server and calendar server name stores their own way.
        let mut offered = Vec::new();
        let folders = store(
            "Shared Folders/support",
            StoreDomains::of(SearchDomain::Mail),
        );
        let principal = store(
            "https://dav.example.org/principals/support/",
            StoreDomains::of(SearchDomain::Calendar),
        );
        merge(&mut offered, folders, SharedUse::Mail);
        merge(&mut offered, principal, SharedUse::Calendar);
        assert_eq!(offered.len(), 2);
    }
}
