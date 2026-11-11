//! Asking a connected account which stores its sign-in opens besides its own, for mail or for
//! calendars (`docs/shared-mailboxes.md`). Nothing here is kept: the answer is the server's, read
//! when Settings asks.
//!
//! Mail is asked of the account's mailbox and calendars of its calendar, because each provider
//! reports the stores it can open: an IMAP sign-in's CalDAV half finds calendar homes its IMAP half
//! knows nothing of, and a Microsoft calendar is checked by a calendar probe, not an Inbox one.

use engine_api::{
    AccountId, ApiError, FailureClass, Provider, SearchDomain, SharedAccount, SharedAccounts,
    SyncError,
};

use crate::App;

/// What an account's sign-in can say about the stores it opens besides its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedStoreOffer {
    /// The account's mailbox or calendar is not connected, so there is nothing to ask yet.
    NotConnected,
    /// The server lists them.
    Listed(Vec<SharedAccount>),
    /// The server lists none, but answers for an address the person types.
    ByAddress,
    /// The server has no way to open one.
    Unsupported,
}

/// What the server said of an address the person typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedStoreLookup {
    /// A store the sign-in opens.
    Found(SharedAccount),
    /// Nothing the sign-in can open answers to it, or it is not an address. The server will not
    /// say which of "no such store" and "not shared with you" it is.
    NotOpenable,
    /// The account's mailbox or calendar is not connected, so there is nothing to ask yet.
    NotConnected,
}

impl<P: Provider> App<P> {
    /// Whether `account` has a live provider for `domain` to ask which stores it opens: what
    /// [`App::shared_store_offer`] answers [`SharedStoreOffer::NotConnected`] without.
    pub async fn has_live_store(&self, account: &AccountId, domain: SearchDomain) -> bool {
        match self.account_handle(account).await {
            Some(handle) => asked_of(&handle, domain).is_some(),
            None => false,
        }
    }

    /// Which stores `account`'s sign-in opens besides its own for `domain`, asked of its live
    /// provider for that domain; only those offering `domain`.
    ///
    /// # Errors
    ///
    /// Returns the engine's error when the server was asked and did not answer.
    pub async fn shared_store_offer(
        &self,
        account: &AccountId,
        domain: SearchDomain,
    ) -> Result<SharedStoreOffer, ApiError> {
        let Some(handle) = self.account_handle(account).await else {
            return Ok(SharedStoreOffer::NotConnected);
        };
        let Some(provider) = asked_of(&handle, domain) else {
            return Ok(SharedStoreOffer::NotConnected);
        };
        match provider.connection_info().capabilities.shared_accounts() {
            SharedAccounts::Unsupported => Ok(SharedStoreOffer::Unsupported),
            SharedAccounts::ByAddress => Ok(SharedStoreOffer::ByAddress),
            SharedAccounts::Enumerable => self
                .engine
                .list_shared_accounts(provider)
                .await
                .map(|listed| SharedStoreOffer::Listed(offering(listed, domain))),
        }
    }

    /// The store at `address` for `domain`, asked of `account`'s live provider for it.
    ///
    /// # Errors
    ///
    /// Returns the engine's error when the server did not answer, or refused the sign-in.
    pub async fn resolve_shared_store(
        &self,
        account: &AccountId,
        domain: SearchDomain,
        address: &str,
    ) -> Result<SharedStoreLookup, ApiError> {
        let Some(handle) = self.account_handle(account).await else {
            return Ok(SharedStoreLookup::NotConnected);
        };
        let Some(provider) = asked_of(&handle, domain) else {
            return Ok(SharedStoreLookup::NotConnected);
        };
        match self.engine.resolve_shared_account(provider, address).await {
            Ok(store) if store.domains.contains(domain) => Ok(SharedStoreLookup::Found(store)),
            Ok(_) | Err(ApiError::InvalidInput(_)) => Ok(SharedStoreLookup::NotOpenable),
            Err(ApiError::Sync(SyncError::Provider(err)))
                if err.class() == FailureClass::Permanent =>
            {
                Ok(SharedStoreLookup::NotOpenable)
            }
            Err(err) => Err(err),
        }
    }
}

/// The provider `account` is asked of for `domain`: its mailbox for mail, its calendar for
/// calendars, none for contacts, whose stores this app does not open.
fn asked_of<P>(account: &crate::Account<P>, domain: SearchDomain) -> Option<&P> {
    match domain {
        SearchDomain::Mail => account.providers.first(),
        SearchDomain::Calendar => account.calendar_provider.as_ref(),
        SearchDomain::Contacts => None,
    }
}

/// The stores in `listed` that offer `domain`. A server lists every store the sign-in opens, a
/// colleague's calendar among them, and one without mail is no mailbox.
fn offering(listed: Vec<SharedAccount>, domain: SearchDomain) -> Vec<SharedAccount> {
    listed
        .into_iter()
        .filter(|store| store.domains.contains(domain))
        .collect()
}

#[cfg(test)]
mod tests {
    use engine_api::{SharedAccountId, StoreDomains};

    use super::*;

    fn store(handle: &str, domains: StoreDomains) -> SharedAccount {
        SharedAccount::new(SharedAccountId::try_from(handle).unwrap(), handle, domains)
    }

    #[test]
    fn each_use_is_offered_the_stores_that_hold_it() {
        let group = store(
            "group",
            StoreDomains::of(SearchDomain::Mail).with(SearchDomain::Calendar),
        );
        let colleague = store("colleague", StoreDomains::of(SearchDomain::Calendar));
        let inbox = store("inbox", StoreDomains::of(SearchDomain::Mail));
        let listed = vec![group.clone(), colleague.clone(), inbox.clone()];
        assert_eq!(
            offering(listed.clone(), SearchDomain::Mail),
            [group.clone(), inbox]
        );
        assert_eq!(offering(listed, SearchDomain::Calendar), [group, colleague]);
    }
}
